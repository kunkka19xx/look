import SwiftUI

/// Seconds that move every `interval`, or one still frame while motion is reduced.
private struct SceneTimeline<Content: View>: View {
    @Environment(\.reducesMotion) private var reducesMotion

    let interval: TimeInterval
    @ViewBuilder let content: (Double) -> Content

    var body: some View {
        TimelineView(.animation(minimumInterval: interval, paused: reducesMotion)) { timeline in
            content(reducesMotion ? 0 : timeline.date.timeIntervalSinceReferenceDate)
        }
    }
}

/// The core's snow over the launcher, in front of a faded figure behind the
/// results: a 🎄 in the split layout, Santa with a gift in the compact one.
struct WinterSceneView: View {
    let snowColor: Color
    /// Times the default snow.
    let snowAmount: Double
    let figure: Figure?

    enum Figure {
        case tree
        case santa
    }

    private static let frameInterval: TimeInterval = 1.0 / 60.0
    /// The core's `CRYSTALS`, in its order.
    private static let crystals = ["❅", "❆"]

    private enum FigureStyle {
        /// Glyph size as a share of the panel's height, and its bounds.
        static let treeSizing: (share: Double, min: CGFloat, max: CGFloat) = (0.24, 96, 170)
        static let santaSizing: (share: Double, min: CGFloat, max: CGFloat) = (0.12, 40, 64)
        static let opacity = 0.55
        /// Solid down to here, then faded out to the ground.
        static let fadeStart = 0.3
        static let cornerInset: CGFloat = 12
        /// Santa stands behind his gift, up and to the left, in glyph sizes.
        static let santaRise = 0.5
        static let santaLean = 0.35
    }

    var body: some View {
        ZStack {
            if let figure {
                figureView(figure)
            }
            SceneTimeline(interval: Self.frameInterval) { seconds in
                Canvas { context, size in
                    drawSnow(in: &context, size: size, seconds: seconds)
                }
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private func figureView(_ figure: Figure) -> some View {
        let sizing = figure == .tree ? FigureStyle.treeSizing : FigureStyle.santaSizing
        return GeometryReader { geo in
            glyphs(figure, size: min(max(geo.size.height * sizing.share, sizing.min), sizing.max))
                // One layer, so the gift does not let Santa show through.
                .compositingGroup()
                .opacity(FigureStyle.opacity)
                .mask(LinearGradient(
                    stops: [
                        .init(color: .black, location: FigureStyle.fadeStart),
                        .init(color: .clear, location: 1),
                    ],
                    startPoint: .top,
                    endPoint: .bottom))
                .padding(.trailing, figure == .santa ? FigureStyle.cornerInset : 0)
                .frame(
                    maxWidth: .infinity,
                    maxHeight: .infinity,
                    alignment: figure == .tree ? .bottom : .bottomTrailing)
        }
    }

    @ViewBuilder
    private func glyphs(_ figure: Figure, size: CGFloat) -> some View {
        switch figure {
        case .tree:
            Text("🎄").font(.system(size: size))
        case .santa:
            ZStack(alignment: .bottomTrailing) {
                Text("🎅")
                    .font(.system(size: size))
                    .padding(.bottom, size * FigureStyle.santaRise)
                    .padding(.trailing, size * FigureStyle.santaLean)
                Text("🎁").font(.system(size: size))
            }
        }
    }

    private func drawSnow(in context: inout GraphicsContext, size: CGSize, seconds: Double) {
        let shading = GraphicsContext.Shading.color(snowColor)
        EngineBridge.shared.forEachSnowflake(
            width: size.width, height: size.height, seconds: seconds, amount: snowAmount
        ) { flake in
            context.opacity = flake.alpha
            if let crystal = flake.crystal, Self.crystals.indices.contains(crystal) {
                context.draw(
                    Text(Self.crystals[crystal]).font(.system(size: flake.radius * 2)).foregroundStyle(snowColor),
                    at: CGPoint(x: flake.x, y: flake.y))
            } else {
                let rect = CGRect(x: flake.x, y: flake.y, width: 0, height: 0).insetBy(dx: -flake.radius, dy: -flake.radius)
                context.fill(Path(ellipseIn: rect), with: shading)
            }
        }
    }
}

/// The core's bokeh behind a surface. A floating card draws its own slice of
/// one panel-sized field, so the cards read as one lit scene.
struct BokehView: View {
    @Environment(\.panelGeometry) private var panel

    private static let frameInterval: TimeInterval = 1.0 / 30.0
    /// The light holds near full to this share of its radius, then fades.
    private static let discShare = 0.6
    private static let discAlphaShare = 0.9

    var body: some View {
        GeometryReader { geo in
            let origin = panel.map { geo.frame(in: .named($0.coordinateSpace)).origin } ?? .zero
            let area = panel?.size ?? geo.size
            SceneTimeline(interval: Self.frameInterval) { seconds in
                Canvas { context, size in
                    context.translateBy(x: -origin.x, y: -origin.y)
                    let visible = CGRect(origin: origin, size: size)
                    EngineBridge.shared.forEachBokehGlow(
                        width: area.width, height: area.height, seconds: seconds
                    ) { glow in
                        let rect = CGRect(origin: glow.center, size: .zero).insetBy(dx: -glow.radius, dy: -glow.radius)
                        guard rect.intersects(visible) else { return }
                        let color = { (alpha: Double) in
                            Color(.sRGB, red: glow.red, green: glow.green, blue: glow.blue, opacity: alpha)
                        }
                        context.fill(Path(ellipseIn: rect), with: .radialGradient(
                            Gradient(stops: [
                                .init(color: color(glow.alpha), location: 0),
                                .init(color: color(glow.alpha * Self.discAlphaShare), location: Self.discShare),
                                .init(color: color(0), location: 1),
                            ]),
                            center: glow.center,
                            startRadius: 0,
                            endRadius: glow.radius))
                    }
                }
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}
