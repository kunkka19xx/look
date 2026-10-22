import AppKit
import SwiftUI

/// SwiftUI view that renders a Quick Look thumbnail of a file path,
/// with a 150 ms dwell debounce and automatic cancellation when the
/// path changes (so rapid arrow-key navigation never spends time
/// rendering intermediate selections).
///
/// Falls back to nothing if QuickLook produces no representation
/// (file too large, unsupported type, unreadable). The caller's
/// existing icon + metadata layout remains visible.
struct QuickLookPreviewImage: View {
    let path: String
    let maxHeight: CGFloat

    @State private var image: NSImage?

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .scaledToFit()
                    .frame(maxWidth: .infinity, maxHeight: maxHeight)
                    .shadow(color: .black.opacity(0.2), radius: 8, x: 0, y: 4)
            } else {
                Color.clear.frame(maxWidth: .infinity, maxHeight: maxHeight)
            }
        }
        .task(id: path) {
            image = nil
            // Dwell: don't render until selection has been stable for
            // 150 ms. `.task(id:)` cancels the prior task on path change,
            // so the sleep just returns early on rapid navigation.
            try? await Task.sleep(nanoseconds: 150_000_000)
            if Task.isCancelled { return }

            let scale = NSScreen.main?.backingScaleFactor ?? 2.0
            // Request size is generous; SwiftUI's .scaledToFit handles
            // final display sizing within the pane. Cap the height to
            // avoid an infinite-size request when the caller passes
            // maxHeight = .infinity (flex layout).
            let requestHeight = maxHeight.isFinite ? maxHeight : 600
            let requestSize = CGSize(width: 800, height: requestHeight)
            let img = await QuickLookPreviewService.shared.thumbnail(
                forPath: path,
                size: requestSize,
                scale: scale
            )
            if Task.isCancelled { return }
            image = img
        }
    }
}
