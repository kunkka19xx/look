import Combine
import Foundation

final class LauncherSearchCoordinator: ObservableObject {
    private let bridge: EngineBridge

    @Published var backendResults: [LauncherResult] = []
    @Published private(set) var isSearching: Bool = false

    private var searchTask: Task<Void, Never>?
    private var latestSearchID: UInt64 = 0

    init(bridge: EngineBridge = .shared) {
        self.bridge = bridge
    }

    func invalidateSearchRequests() {
        latestSearchID &+= 1
        searchTask?.cancel()
        searchTask = nil
    }

    func beginSearchRequest() -> UInt64 {
        latestSearchID &+= 1
        return latestSearchID
    }

    func refreshSearchResults(
        query: String,
        isCommandMode: Bool,
        isClipboardQuery: Bool,
        onComplete: @escaping ([LauncherResult]) -> Void
    ) {
        guard !isCommandMode else { return }
        guard !isClipboardQuery else {
            invalidateSearchRequests()
            onComplete([])
            return
        }

        let currentQuery = query
        let searchLimit = AppConstants.Launcher.defaultSearchLimit
        let searchID = beginSearchRequest()
        let bridge = self.bridge
        searchTask?.cancel()
        searchTask = Task {
            try? await Task.sleep(nanoseconds: AppConstants.Launcher.searchDebounceNanoseconds)
            guard !Task.isCancelled else { return }

            let results = await Task.detached(priority: .userInitiated) {
                bridge.search(query: currentQuery, limit: searchLimit)
            }.value

            await MainActor.run {
                guard searchID == latestSearchID else { return }
                guard !isCommandMode, query == currentQuery else { return }
                backendResults = results
                onComplete(results)
            }
        }
    }

    func cancel() {
        searchTask?.cancel()
    }
}
