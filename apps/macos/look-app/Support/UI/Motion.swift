import SwiftUI

/// Shared motion constants for the launcher, so every animated surface reads with
/// the same physics and the feel is tuned in one place.
enum Motion {
    /// The single highlight pill gliding between rows on keyboard navigation.
    enum Selection {
        static let geometryID = "look.selection.pill"
        static let response: Double = 0.3
        static let dampingFraction: Double = 0.85

        /// One-shot zoom as a row takes the selection. The icon needs a much
        /// larger factor than the pill: at 22pt a few percent is one point,
        /// while the same on a full-width pill looks like the layout breathing.
        static let iconZoomScale: CGFloat = 1.24
        static let pillZoomScale: CGFloat = 1.02
        static let zoomInSeconds: Double = 0.11
        static let zoomOutResponse: Double = 0.3
        static let zoomOutDamping: Double = 0.6

        /// Horizontal offset held by the selected row's text while selected.
        static let titleShift: CGFloat = 4

        static var glide: Animation {
            .spring(response: response, dampingFraction: dampingFraction)
        }

        static var zoomIn: Animation {
            .easeOut(duration: zoomInSeconds)
        }

        static var zoomOut: Animation {
            .spring(response: zoomOutResponse, dampingFraction: zoomOutDamping)
        }
    }

    /// The search field's snowman on every open: it zooms up and back while
    /// it shakes side to side, settling as the window lands.
    enum Snowman {
        struct Pose {
            var scale: CGFloat = 1
            var angle: Double = 0
        }

        static let peakScale: CGFloat = 1.35
        static let zoomInSeconds: Double = 0.1
        static let settleSeconds: Double = 0.2
        /// Degrees, each swing smaller than the last.
        static let shakeAngles: [Double] = [-14, 12, -8, 4, 0]
        static let shakeStepSeconds: Double = 0.05
    }

    /// The press-down on an interactive surface (result rows, tiles), so a
    /// click reads as physical rather than instant.
    enum Press {
        static let scale: CGFloat = 0.975
        static let response: Double = 0.24
        static let dampingFraction: Double = 0.9

        static var animation: Animation {
            .spring(response: response, dampingFraction: dampingFraction)
        }
    }

    /// Icon and value changes: a toggle flipping, a counter ticking.
    enum Value {
        /// Digit roll for a readout that ticks (battery, temperature, timers).
        /// Kept under the one-second tick of the fastest caller (the pomo
        /// countdown) so a roll always settles before the next value lands.
        static let rollSeconds: Double = 0.28

        static var rollDigits: Animation {
            .easeInOut(duration: rollSeconds)
        }
    }

    /// A row landing in or leaving a list: a skipped folder, an extra scan
    /// directory. Springs so an entry reads as placed rather than popped in.
    enum Insert {
        static let response: Double = 0.34
        static let dampingFraction: Double = 0.82
        /// Grown from, and collapsed back to, on the row's leading edge.
        static let startScale: CGFloat = 0.9

        static var animation: Animation {
            .spring(response: response, dampingFraction: dampingFraction)
        }

        static var transition: AnyTransition {
            .scale(scale: startScale, anchor: .leading).combined(with: .opacity)
        }
    }

    /// Content changing in place rather than moving: the pomo panel dimming to
    /// its idle state, a chosen path replacing the empty-state line.
    enum Fade {
        static let seconds: Double = 0.4

        static var animation: Animation {
            .easeInOut(duration: seconds)
        }
    }

    /// The launcher zooming up and fading in when it opens, and back down when
    /// it closes. Played by the window server and the render server on
    /// purpose, so no frame waits on a SwiftUI render.
    enum Surface {
        static let arriveSeconds: TimeInterval = 0.24
        static let leaveSeconds: TimeInterval = 0.14
        static let restingScale: CGFloat = 1
        static let awayScale: CGFloat = 0.92
        /// One key for both directions, so an open replaces a close under way.
        static let zoomKey = "look.surface.zoom"
    }

    /// The gliding search-field caret (see `SmoothCaretTextField`).
    enum Caret {
        static let width: CGFloat = 2
        static let cornerRadius: CGFloat = 1
        /// Multiplier on the font's line height so the bar reads as a caret.
        static let heightScale: CGFloat = 1.05
        static let glideSeconds: CFTimeInterval = 0.105
        static let blinkPeriodSeconds: CFTimeInterval = 1.05
        /// Idle time after a keystroke before the blink resumes (solid while typing).
        static let blinkResumeSeconds: TimeInterval = 0.4
    }

    /// House easing as cubic-bezier control points, for what Core Animation
    /// plays: the caret and the window zoom.
    static let houseCurveControlPoints: (Float, Float, Float, Float) = (0.22, 1, 0.36, 1)

    /// The one rule for whether motion plays, for SwiftUI and AppKit alike.
    static func isReduced(bySystem systemReducesMotion: Bool, animationsEnabled: Bool) -> Bool {
        systemReducesMotion || !animationsEnabled
    }
}

private struct ReducesMotionKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    /// True when macOS Reduce Motion is on or the user turned Look's
    /// animations off. Read this instead of `accessibilityReduceMotion`.
    var reducesMotion: Bool {
        get { self[ReducesMotionKey.self] }
        set { self[ReducesMotionKey.self] = newValue }
    }
}

/// Publishes `reducesMotion` and, when animations are off, strips the
/// animation from every transaction below, `withAnimation` and implicit
/// `.animation(_:value:)` alike.
private struct MotionPreference: ViewModifier {
    let animationsEnabled: Bool
    @Environment(\.accessibilityReduceMotion) private var systemReducesMotion

    func body(content: Content) -> some View {
        content
            .environment(
                \.reducesMotion,
                Motion.isReduced(bySystem: systemReducesMotion, animationsEnabled: animationsEnabled)
            )
            .transaction { transaction in
                guard !animationsEnabled else { return }
                transaction.animation = nil
                transaction.disablesAnimations = true
            }
    }
}

/// Scales a surface down while it is held, so clicks read as physical.
struct PressableSurfaceStyle: ButtonStyle {
    @Environment(\.reducesMotion) private var reduceMotion

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed && !reduceMotion ? Motion.Press.scale : 1)
            .animation(Motion.Press.animation, value: configuration.isPressed)
    }
}

extension View {
    /// Applies the user's animation preference to the whole subtree.
    func motionPreference(animationsEnabled: Bool) -> some View {
        modifier(MotionPreference(animationsEnabled: animationsEnabled))
    }
}
