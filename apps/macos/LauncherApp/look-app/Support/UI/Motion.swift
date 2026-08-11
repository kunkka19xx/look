import SwiftUI

/// Shared motion constants for the launcher, so every animated surface reads with
/// the same physics and the feel is tuned in one place.
enum Motion {
    /// The "materialize on open" reveal: tiles start slightly enlarged and
    /// transparent, then spring to rest, staggered so the grid settles in.
    enum Spawn {
        static let startScale: CGFloat = 1.05
        static let staggerSeconds: Double = 0.02
        /// Ceiling on the cascade so a large grid never trails on too long.
        static let maxStaggerSeconds: Double = 0.22
        static let response: Double = 0.42
        static let dampingFraction: Double = 0.82

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

        static var glide: Animation {
            .spring(response: response, dampingFraction: dampingFraction)
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
        /// Fade applied to a divider that yields to the selection pill.
        static let dividerFadeSeconds: Double = 0.18
        /// Digit roll for a readout that ticks (battery, temperature, timers).
        /// Kept under the one-second tick of the fastest caller (the pomo
        /// countdown) so a roll always settles before the next value lands.
        static let rollSeconds: Double = 0.28

        static var dividerFade: Animation {
            .easeOut(duration: dividerFadeSeconds)
        }

        static var rollDigits: Animation {
            .easeInOut(duration: rollSeconds)
        }
    }

    /// The whole panel arriving when the launcher opens, and the surfaces inside
    /// it swapping as the query changes.
    enum Surface {
        /// The panel lands from slightly small and transparent. Deliberately a
        /// content-layer effect: animating the window itself would mean touching
        /// the `makeKeyAndOrderFront` path that the Cmd+Space cold-login bug
        /// lives in, and this reads the same from the outside.
        static let arriveScale: CGFloat = 0.965
        static let arriveResponse: Double = 0.34
        static let arriveDamping: Double = 0.86
        /// Launchpad to results and back. Faster than the arrival, since it fires
        /// on the first keystroke and must not feel like lag.
        static let swapSeconds: Double = 0.2

        static var arrive: Animation {
            .spring(response: arriveResponse, dampingFraction: arriveDamping)
        }

        static var swap: Animation {
            .easeOut(duration: swapSeconds)
        }

        /// Surfaces cross-dissolve with a touch of scale rather than cutting.
        static var swapTransition: AnyTransition {
            .opacity.combined(with: .scale(scale: 0.99))
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
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .scaleEffect(shown ? 1 : Motion.Spawn.startScale)
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

/// Fades and scales the whole panel in each time the launcher is shown. The
/// window is only ordered out and back in, so this keys off the same token the
/// tile cascade uses rather than `onAppear`, and the panel arrives just ahead of
/// the tiles settling into it.
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
        // Two transactions, same reason as `SpawnReveal`: the hidden state has to
        // land before the animated one or there is no "from" value to move off.
        shown = false
        DispatchQueue.main.async {
            withAnimation(Motion.Surface.arrive) {
                shown = true
            }
        }
    }
}

/// Scales a surface down while it is held, so clicks read as physical. Under
/// Reduce Motion the surface holds its resting size and only the hit behaviour
/// remains.
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
    func spawnReveal(index: Int, token: UInt64) -> some View {
        modifier(SpawnReveal(index: index, token: token))
    }

    /// Fades and scales the panel in on every launcher open. `token` is the same
    /// counter that replays the tile cascade.
    func rootReveal(token: UInt64) -> some View {
        modifier(RootReveal(token: token))
    }
}
