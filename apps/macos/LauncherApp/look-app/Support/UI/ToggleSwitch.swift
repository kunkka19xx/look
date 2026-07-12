import SwiftUI

/// A macOS-style toggle switch: a capsule track with a sliding knob, green when
/// on. `isOn == nil` renders a dimmed, disabled switch (e.g. while state loads).
/// Reusable across the app; the Quick Actions panel uses it for toggle controls.
struct ToggleSwitch: View {
    let isOn: Bool?
    var themeStore: ThemeStore
    var onToggle: () -> Void = {}

    private enum Layout {
        static let trackWidth: CGFloat = 42
        static let trackHeight: CGFloat = 24
        static let knobSize: CGFloat = 20
        static let knobPadding: CGFloat = 2
        static let knobShadowRadius: CGFloat = 1
        static let animationDuration = 0.15
        static let loadingOpacity = 0.5
    }

    private enum Track {
        static let onOpacity = 0.9
        static let offOpacity = 0.55
        static let loadingOpacity = 0.35
    }

    var body: some View {
        Button(action: onToggle) {
            ZStack(alignment: isOn == true ? .trailing : .leading) {
                Capsule()
                    .fill(trackColor)
                    .frame(width: Layout.trackWidth, height: Layout.trackHeight)
                Circle()
                    .fill(.white)
                    .frame(width: Layout.knobSize, height: Layout.knobSize)
                    .padding(Layout.knobPadding)
                    .shadow(color: .black.opacity(0.25), radius: Layout.knobShadowRadius, x: 0, y: 0.5)
            }
        }
        .buttonStyle(.plain)
        .disabled(isOn == nil)
        .opacity(isOn == nil ? Layout.loadingOpacity : 1)
        .animation(.easeInOut(duration: Layout.animationDuration), value: isOn)
    }

    private var trackColor: Color {
        switch isOn {
        case true: return Color.green.opacity(Track.onOpacity)
        case false: return themeStore.dividerColor().opacity(Track.offOpacity)
        case nil: return themeStore.dividerColor().opacity(Track.loadingOpacity)
        }
    }
}
