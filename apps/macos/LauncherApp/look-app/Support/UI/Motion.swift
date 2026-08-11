import SwiftUI

/// Shared motion constants for the launcher, so every animated surface reads with
/// the same physics and the feel is tuned in one place.
enum Motion {
    /// The "materialize on open" reveal: tiles grow and rise into place,
    /// staggered so the grid assembles rather than appearing at once.
    enum Spawn {
        static let startScale: CGFloat = 0.88
        static let startOffsetY: CGFloat = 14
        static let staggerSeconds: Double = 0.035
        /// Ceiling on the cascade so a large grid never trails on too long.
        static let maxStaggerSeconds: Double = 0.34
        static let response: Double = 0.46
        /// Under 1 to overshoot slightly and settle back.
        static let dampingFraction: Double = 0.68

        static func animation(index: Int) -> Animation {
            let delay = min(Double(max(0, index)) * staggerSeconds, maxStaggerSeconds)
            return .spring(response: response, dampingFraction: dampingFraction).delay(delay)
        }
    }

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

    /// The whole panel arriving when the launcher opens. A content-layer effect
    /// on purpose: animating the window would mean touching the
    /// `makeKeyAndOrderFront` path the Cmd+Space cold-login bug lives in.
    enum Surface {
        static let arriveScale: CGFloat = 0.965
        static let arriveResponse: Double = 0.34
        static let arriveDamping: Double = 0.86

        static var arrive: Animation {
            .spring(response: arriveResponse, dampingFraction: arriveDamping)
        }
    }

    /// Horizontal slide-ins on open: the search placeholder from the right, the
    /// running-apps strip from the left.
    enum Slide {
        static let placeholderOffsetX: CGFloat = 20
        static let stripOffsetX: CGFloat = -18
        static let startScale: CGFloat = 0.92
        /// The spring's period: the main dial for how fast these read.
        static let response: Double = 0.8
        /// Loose enough to carry a little past 1 and settle back.
        static let dampingFraction: Double = 0.76
        /// Lands just behind the bar these sit in.
        static let delaySeconds: Double = 0.09
        static let staggerSeconds: Double = 0.04
        static let maxStaggerSeconds: Double = 0.28

        static func arrive(index: Int) -> Animation {
            let stagger = min(Double(max(0, index)) * staggerSeconds, maxStaggerSeconds)
            return .spring(response: response, dampingFraction: dampingFraction)
                .delay(delaySeconds + stagger)
        }
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

    /// House easing as cubic-bezier control points, for the CoreAnimation caret
    /// (the spring-driven motion above approximates the same feel natively).
    static let houseCurveControlPoints: (Float, Float, Float, Float) = (0.22, 1, 0.36, 1)
}

/// Plays the spawn reveal on appear and whenever `token` changes. The launcher
/// bumps `token` on every window show, so the cascade replays each open (the
/// window is only ordered out, so `onAppear` alone fires once per process).
private struct SpawnReveal: ViewModifier {
    let index: Int
    let token: UInt64
    /// Off for surfaces holding an `NSViewRepresentable`: scaling the search
    /// field softens its text for the length of the animation.
    var scales: Bool = true
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .scaleEffect(shown || !scales ? 1 : Motion.Spawn.startScale)
            .offset(y: shown ? 0 : Motion.Spawn.startOffsetY)
            .onAppear { rearm() }
            .onChange(of: token) { _, _ in rearm() }
    }

    private func rearm() {
        if reduceMotion {
            shown = true
            return
        }
        // Snap to hidden with no animation, then animate to shown on the next
        // runloop tick: the two states need separate transactions to capture a
        // "from" value and actually animate.
        shown = false
        DispatchQueue.main.async {
            withAnimation(Motion.Spawn.animation(index: index)) {
                shown = true
            }
        }
    }
}

/// Fades and scales the whole panel in on every show. Keys off the same token
/// as `SpawnReveal`, since the window is only ordered out and back in.
private struct RootReveal: ViewModifier {
    let token: UInt64
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .scaleEffect(shown ? 1 : Motion.Surface.arriveScale)
            .onAppear { rearm() }
            .onChange(of: token) { _, _ in rearm() }
    }

    private func rearm() {
        if reduceMotion {
            shown = true
            return
        }
        // Two transactions, same reason as `SpawnReveal`.
        shown = false
        DispatchQueue.main.async {
            withAnimation(Motion.Surface.arrive) {
                shown = true
            }
        }
    }
}

/// Slides a view in horizontally and settles it, on every launcher open.
/// `index` places it in the stagger; a negative `startOffsetX` comes from the
/// left, a positive one from the right.
private struct SlideReveal: ViewModifier {
    let index: Int
    let token: UInt64
    let startOffsetX: CGFloat
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .scaleEffect(shown ? 1 : Motion.Slide.startScale)
            .offset(x: shown ? 0 : startOffsetX)
            .onAppear { rearm() }
            .onChange(of: token) { _, _ in rearm() }
    }

    private func rearm() {
        if reduceMotion {
            shown = true
            return
        }
        // Two transactions, same reason as `SpawnReveal`.
        shown = false
        DispatchQueue.main.async {
            withAnimation(Motion.Slide.arrive(index: index)) {
                shown = true
            }
        }
    }
}
/// Scales a surface down while it is held, so clicks read as physical.
struct PressableSurfaceStyle: ButtonStyle {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed && !reduceMotion ? Motion.Press.scale : 1)
            .animation(Motion.Press.animation, value: configuration.isPressed)
    }
}

extension View {
    /// Reveals the view with the shared spawn cascade. `index` sets its place in the
    /// stagger; changing `token` replays the reveal (the launcher bumps it on show).
    func spawnReveal(index: Int, token: UInt64, scales: Bool = true) -> some View {
        modifier(SpawnReveal(index: index, token: token, scales: scales))
    }

    /// Fades and scales the panel in on every launcher open.
    func rootReveal(token: UInt64) -> some View {
        modifier(RootReveal(token: token))
    }
    /// Slides the search placeholder in from the right on every launcher open.
    func placeholderReveal(token: UInt64) -> some View {
        modifier(SlideReveal(index: 0, token: token, startOffsetX: Motion.Slide.placeholderOffsetX))
    }

    /// Slides a running-apps icon in from the left, staggered by `index`.
    func stripReveal(index: Int, token: UInt64) -> some View {
        modifier(SlideReveal(index: index, token: token, startOffsetX: Motion.Slide.stripOffsetX))
    }
}
