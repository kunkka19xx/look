import SwiftUI

/// Shared motion constants for the launcher, so every animated surface reads with
/// the same physics and the feel is tuned in one place.
enum Motion {
    /// The "materialize on open" reveal: tiles start slightly enlarged and
    /// transparent, then spring to rest, staggered so the grid settles in.
    enum Spawn {
        /// Tiles grow into place rather than shrinking into it: arriving reads as
        /// coming toward the viewer, and the old 1.05 start was too near 1 to see.
        static let startScale: CGFloat = 0.88
        /// Each tile also rises the last few points into its slot, which is what
        /// makes the grid look like it assembles rather than simply appearing.
        static let startOffsetY: CGFloat = 14
        static let staggerSeconds: Double = 0.035
        /// Ceiling on the cascade so a large grid never trails on too long.
        static let maxStaggerSeconds: Double = 0.34
        static let response: Double = 0.46
        /// Under 1 by enough to overshoot slightly and settle back. That small
        /// bounce is most of what reads as "alive" rather than "faded in".
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

        /// After the pill glides into place it zooms once and settles. The glide
        /// alone is hard to follow when the travel is a single row, and this is
        /// the only thing on screen that should move during keyboard nav.
        ///
        /// The icon and the pill need very different numbers: the icon is 22pt,
        /// so a few percent is a change of one point and reads as nothing, while
        /// the pill spans the whole row and the same percentage would look like
        /// the layout breathing.
        static let iconZoomScale: CGFloat = 1.24
        static let pillZoomScale: CGFloat = 1.02
        static let zoomInSeconds: Double = 0.11
        static let zoomOutResponse: Double = 0.3
        /// Loose enough to overshoot slightly coming back to rest.
        static let zoomOutDamping: Double = 0.6

        /// How far the selected row's text slides in from. Held while the row is
        /// selected rather than sprung back, so the offset also reads as part of
        /// the selection rather than only as a twitch on arrival.
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

        static var arrive: Animation {
            .spring(response: arriveResponse, dampingFraction: arriveDamping)
        }
    }

    /// The bounce `symbolEffect(.bounce)` gives an SF Symbol, rebuilt as a plain
    /// scale pop so bitmap icons (app icons are `NSImage`, not symbols) can join
    /// the same moment.
    enum Pop {
        static let peakScale: CGFloat = 1.16
        static let riseSeconds: Double = 0.13
        static let settleResponse: Double = 0.34
        /// Loose enough to overshoot on the way back, matching the symbol bounce.
        static let settleDamping: Double = 0.55

        static var rise: Animation {
            .easeOut(duration: riseSeconds)
        }

        static var settle: Animation {
            .spring(response: settleResponse, dampingFraction: settleDamping)
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

/// Pops a non-symbol icon on every launcher open, so bitmap icons bounce
/// alongside the SF Symbols that `symbolEffect(.bounce)` handles.
private struct IconPop: ViewModifier {
    let token: UInt64
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var popped = false

    func body(content: Content) -> some View {
        content
            .scaleEffect(popped ? Motion.Pop.peakScale : 1)
            .onChange(of: token) { _, _ in fire() }
    }

    private func fire() {
        guard !reduceMotion else { return }
        withAnimation(Motion.Pop.rise) {
            popped = true
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Motion.Pop.riseSeconds) {
            withAnimation(Motion.Pop.settle) {
                popped = false
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

    /// Bounces a bitmap icon when `token` changes, matching what
    /// `symbolEffect(.bounce)` does for SF Symbols.
    func iconPop(token: UInt64) -> some View {
        modifier(IconPop(token: token))
    }
}
