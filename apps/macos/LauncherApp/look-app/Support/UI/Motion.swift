import SwiftUI

/// Shared motion vocabulary for the launcher. Kept in one place so every animated
/// surface (tile spawn cascade, selection glide) reads with the same physics, and
/// the feel can be tuned here instead of hunting numbers through the views.
///
/// The launcher leans on native SwiftUI springs rather than porting the linows CSS
/// curves one for one: springs give the "settle" the empty query reveal is after
/// (modelled on the iOS unlock, where icons start slightly enlarged and settle to
/// their resting size) without hand tuned keyframes.
enum Motion {
    /// The "materialize on open" reveal for launchpad / quick action tiles. Tiles
    /// start slightly enlarged and transparent, then spring down to rest, cascaded
    /// by a per tile delay so the grid settles in instead of popping all at once.
    enum Spawn {
        /// Scale a tile starts at before settling to 1.0 (the unlock zoom out).
        static let startScale: CGFloat = 1.05
        /// Delay added per tile so the grid cascades rather than popping at once.
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

    /// The selection highlight gliding between rows on keyboard navigation. Fast
    /// enough to feel responsive to held arrow keys, damped enough not to wobble.
    enum Selection {
        /// Shared `matchedGeometryEffect` id for the single sliding pill.
        static let geometryID = "look.selection.pill"
        static let response: Double = 0.3
        static let dampingFraction: Double = 0.85

        static var glide: Animation {
            .spring(response: response, dampingFraction: dampingFraction)
        }
    }

    /// The gliding "monkeytype" caret in the search field. `SmoothCaretTextField`
    /// suppresses the native insertion point and animates a bar to the real caret
    /// position, so the cursor slides as you type instead of jumping.
    enum Caret {
        static let width: CGFloat = 2
        static let cornerRadius: CGFloat = 1
        /// Added to the font's line height so the bar reads as a caret, not a full
        /// line-height block.
        static let heightScale: CGFloat = 1.05
        /// How long the bar takes to glide to a new caret position.
        static let glideSeconds: CFTimeInterval = 0.105
        /// One full blink cycle while the field is idle.
        static let blinkPeriodSeconds: CFTimeInterval = 1.05
        /// Idle time after the last keystroke before the blink resumes; the bar
        /// stays solid while actively typing.
        static let blinkResumeSeconds: TimeInterval = 0.4
    }

    /// House easing (linows' signature glide) as cubic-bezier control points, used
    /// where an explicit curve is needed (the CoreAnimation caret). The spring
    /// driven spawn / selection motion approximates the same feel natively.
    static let houseCurveControlPoints: (Float, Float, Float, Float) = (0.22, 1, 0.36, 1)
}

/// Plays the spawn reveal when the view first appears and again whenever `token`
/// changes. The launcher re-arms this on every window show, so the cascade replays
/// each time Look opens (the AppKit window is only ordered out, never torn down, so
/// `onAppear` alone would fire just once per process).
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
        // Snap to the hidden frame with no animation, then let the next runloop
        // tick animate to shown: SwiftUI needs the two states in separate
        // transactions to capture a "from" value (the reflow linows forces in JS).
        shown = false
        DispatchQueue.main.async {
            withAnimation(Motion.Spawn.animation(index: index)) {
                shown = true
            }
        }
    }
}

extension View {
    /// Reveals the view with the shared spawn cascade. `index` sets its place in the
    /// stagger; changing `token` replays the reveal (the launcher bumps it on show).
    func spawnReveal(index: Int, token: UInt64) -> some View {
        modifier(SpawnReveal(index: index, token: token))
    }
}
