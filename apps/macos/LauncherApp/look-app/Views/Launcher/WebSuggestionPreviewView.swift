import SwiftUI

/// Preview pane for a selected Google autocomplete row. A search suggestion has
/// no file metadata, so instead of the generic file preview we show a simple
/// "search the web" action card.
struct WebSuggestionPreviewView: View {
    let query: String
    @ObservedObject var themeStore: ThemeStore

    private var fontSize: CGFloat { CGFloat(themeStore.settings.fontSize) }

    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 44, weight: .light))
                .foregroundStyle(themeStore.accentColor())

            Text(query)
                .font(themeStore.uiFont(size: fontSize + 3, weight: .semibold))
                .foregroundStyle(themeStore.fontColor())
                .multilineTextAlignment(.center)
                .lineLimit(3)
                .textSelection(.enabled)

            Text("Search Google")
                .font(themeStore.uiFont(size: fontSize - 1, weight: .semibold))
                .foregroundStyle(themeStore.mutedTextColor())

            HStack(spacing: 6) {
                Text("Press")
                Text("Enter")
                    .fontWeight(.semibold)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 1)
                    .background(.white.opacity(0.12), in: RoundedRectangle(cornerRadius: 4, style: .continuous))
                Text("to search the web")
            }
            .font(themeStore.uiFont(size: fontSize - 2, weight: .regular))
            .foregroundStyle(themeStore.secondaryTextColor())
        }
        .padding(20)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
