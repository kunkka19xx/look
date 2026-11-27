import AppKit
import QuartzCore
import SwiftUI

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
            let color = NSColor(snowColor).cgColor
            SceneLayer(makePainter: SnowPainter.init) { $0.configure(color: color, amount: snowAmount) }
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
                .mask(
                    LinearGradient(
                        stops: [
                            .init(color: .black, location: FigureStyle.fadeStart),
                            .init(color: .clear, location: 1),
                        ],
                        startPoint: .top,
                        endPoint: .bottom)
                )
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
}

/// The core's bokeh behind a surface. A floating card draws its own slice of
/// one panel-sized field, so the cards read as one lit scene.
struct BokehView: View {
    @Environment(\.panelGeometry) private var panel

    var body: some View {
        GeometryReader { geo in
            let origin = panel.map { geo.frame(in: .named($0.coordinateSpace)).origin } ?? .zero
            let area = panel?.size ?? geo.size
            SceneLayer(makePainter: BokehPainter.init) { $0.place(origin: origin, area: area) }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

// MARK: - Layer scenes

/// A painter's scene as a view. `configure` hands the painter this render's
/// inputs and answers whether any of them changed.
private struct SceneLayer<Painter: ScenePainter>: NSViewRepresentable {
    let makePainter: () -> Painter
    let configure: (Painter) -> Bool

    func makeCoordinator() -> Painter { makePainter() }

    func makeNSView(context: Context) -> SceneLayerView {
        SceneLayerView(painter: context.coordinator)
    }

    func updateNSView(_ view: SceneLayerView, context: Context) {
        view.reducesMotion = context.environment.reducesMotion
        if configure(context.coordinator) {
            view.refresh()
        }
    }
}

private protocol ScenePainter: AnyObject {
    var framesPerSecond: Float { get }
    func paint(in canvas: CALayer, size: CGSize, seconds: Double)
}

/// Every key a scene frame changes, so none of them animates implicitly.
private let stillLayerActions: [String: any CAAction] = Dictionary(
    uniqueKeysWithValues: [
        #keyPath(CALayer.position), #keyPath(CALayer.bounds), #keyPath(CALayer.opacity),
        #keyPath(CALayer.isHidden), #keyPath(CALayer.backgroundColor), #keyPath(CALayer.cornerRadius),
        #keyPath(CALayer.contentsScale), #keyPath(CALayer.sublayers), #keyPath(CAGradientLayer.colors),
        #keyPath(CATextLayer.string), #keyPath(CATextLayer.fontSize), #keyPath(CATextLayer.foregroundColor),
        kCAOnOrderIn,
    ].map { ($0, NSNull()) })

/// Runs a painter on its screen's display link while its window is visible.
/// A frame only moves Core Animation layers and never re-renders SwiftUI,
/// which would lay out the whole launcher and reset the cursor every time.
private final class SceneLayerView: NSView {
    let painter: any ScenePainter
    var reducesMotion = false {
        didSet {
            if reducesMotion != oldValue { syncLink() }
        }
    }

    private let canvas = CALayer()
    private var link: CADisplayLink?
    private var occlusionObserver: (any NSObjectProtocol)?

    init(painter: any ScenePainter) {
        self.painter = painter
        super.init(frame: .zero)
        wantsLayer = true
        canvas.masksToBounds = true
        canvas.actions = stillLayerActions
        layer?.addSublayer(canvas)
        setAccessibilityElement(false)
    }

    required init?(coder: NSCoder) {
        fatalError("SceneLayerView is created in code only")
    }

    override var isFlipped: Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func layout() {
        super.layout()
        canvas.frame = bounds
        paintFrame()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        canvas.contentsScale = window?.backingScaleFactor ?? canvas.contentsScale
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        link?.invalidate()
        link = nil
        if let occlusionObserver {
            NotificationCenter.default.removeObserver(occlusionObserver)
            self.occlusionObserver = nil
        }
        guard let window else { return }

        canvas.contentsScale = window.backingScaleFactor
        let link = displayLink(target: self, selector: #selector(step))
        let rate = painter.framesPerSecond
        link.preferredFrameRateRange = CAFrameRateRange(minimum: rate, maximum: rate, preferred: rate)
        link.add(to: .main, forMode: .common)
        self.link = link
        occlusionObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didChangeOcclusionStateNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.syncLink() }
        }
        syncLink()
    }

    /// Repaints now when no running link would pick a change up next frame.
    func refresh() {
        if link?.isPaused != false {
            paintFrame()
        }
    }

    private func syncLink() {
        let isVisible = window?.occlusionState.contains(.visible) == true
        link?.isPaused = reducesMotion || !isVisible
        refresh()
    }

    @objc private func step(_ link: CADisplayLink) {
        paintFrame()
    }

    private func paintFrame() {
        guard !bounds.isEmpty else { return }
        painter.paint(
            in: canvas,
            size: bounds.size,
            seconds: reducesMotion ? 0 : Date.timeIntervalSinceReferenceDate)
    }
}

/// One kind of scene layer, reused frame to frame in its own container and
/// restyled only when what it shows changes.
///
/// A struct: the Release optimizer crashes on a generic class's deinit.
private struct LayerPool<Layer: CALayer, Style: Equatable> {
    private let container = CALayer()
    private let make: () -> Layer
    private var layers: [Layer] = []
    private var styles: [Style?] = []
    private var used = 0

    init(make: @escaping () -> Layer) {
        self.make = make
        container.actions = stillLayerActions
    }

    func attach(to canvas: CALayer) {
        if container.superlayer !== canvas {
            canvas.addSublayer(container)
        }
        if container.frame != canvas.bounds {
            container.frame = canvas.bounds
        }
    }

    /// Shows this frame's next layer at `center`, restyling it first when needed.
    mutating func place(style: Style, at center: CGPoint, opacity: Double, apply: (Layer) -> Void) {
        if used == layers.count {
            let layer = make()
            layer.actions = stillLayerActions
            container.addSublayer(layer)
            layers.append(layer)
            styles.append(nil)
        }
        let layer = layers[used]
        if styles[used] != style {
            apply(layer)
            styles[used] = style
        }
        if layer.isHidden {
            layer.isHidden = false
        }
        layer.position = center
        layer.opacity = Float(opacity)
        used += 1
    }

    /// Hides what this frame left unused and starts the next one.
    mutating func finishFrame() {
        for layer in layers[used...] where !layer.isHidden {
            layer.isHidden = true
        }
        used = 0
    }

    mutating func restyleAll() {
        styles = Array(repeating: nil, count: styles.count)
    }
}

/// The core's snowfall: round dots, with the rare crystal glyph above them.
private final class SnowPainter: ScenePainter {
    let framesPerSecond: Float = 60

    /// The core's `CRYSTALS`, in its order.
    private static let crystalGlyphs = ["❅", "❆"]

    private struct CrystalStyle: Equatable {
        let glyph: Int
        let radius: Double
        let scale: CGFloat
    }

    private var color = CGColor.white
    private var amount = 1.0
    private var dots = LayerPool<CALayer, Double> { CALayer() }
    private var crystals = LayerPool<CATextLayer, CrystalStyle> {
        let layer = CATextLayer()
        layer.alignmentMode = .center
        return layer
    }

    /// Whether the snow changed, so a still frame needs repainting.
    func configure(color: CGColor, amount: Double) -> Bool {
        guard color != self.color || amount != self.amount else { return false }
        if color != self.color {
            dots.restyleAll()
            crystals.restyleAll()
        }
        self.color = color
        self.amount = amount
        return true
    }

    func paint(in canvas: CALayer, size: CGSize, seconds: Double) {
        dots.attach(to: canvas)
        crystals.attach(to: canvas)
        EngineBridge.shared.forEachSnowflake(
            width: size.width, height: size.height, seconds: seconds, amount: amount
        ) { flake in
            let center = CGPoint(x: flake.x, y: flake.y)
            let diameter = flake.radius * 2
            if let glyph = flake.crystal, Self.crystalGlyphs.indices.contains(glyph) {
                let style = CrystalStyle(glyph: glyph, radius: flake.radius, scale: canvas.contentsScale)
                crystals.place(style: style, at: center, opacity: flake.alpha) { layer in
                    let text = Self.crystalGlyphs[glyph]
                    let font = NSFont.systemFont(ofSize: diameter)
                    layer.string = text
                    layer.font = font
                    layer.fontSize = diameter
                    layer.foregroundColor = color
                    layer.contentsScale = style.scale
                    layer.bounds = CGRect(
                        origin: .zero, size: NSAttributedString(string: text, attributes: [.font: font]).size())
                }
            } else {
                dots.place(style: flake.radius, at: center, opacity: flake.alpha) { layer in
                    layer.backgroundColor = color
                    layer.cornerRadius = flake.radius
                    layer.bounds = CGRect(x: 0, y: 0, width: diameter, height: diameter)
                }
            }
        }
        dots.finishFrame()
        crystals.finishFrame()
    }
}

/// The core's bokeh: each glow a radial gradient, faded through its opacity.
private final class BokehPainter: ScenePainter {
    let framesPerSecond: Float = 30

    /// The light holds near full to this share of its radius, then fades.
    private static let discShare = 0.6
    private static let discAlphaShare = 0.9
    private static let gradientCenter = CGPoint(x: 0.5, y: 0.5)
    private static let gradientEdge = CGPoint(x: 1, y: 1)

    private struct GlowStyle: Equatable {
        let red: Double
        let green: Double
        let blue: Double
        let radius: Double
    }

    private var origin = CGPoint.zero
    private var area = CGSize.zero
    private var glows = LayerPool<CAGradientLayer, GlowStyle> {
        let layer = CAGradientLayer()
        layer.type = .radial
        layer.startPoint = BokehPainter.gradientCenter
        layer.endPoint = BokehPainter.gradientEdge
        layer.locations = [0, NSNumber(value: BokehPainter.discShare), 1]
        return layer
    }

    /// Whether this slice of the field moved, so a still frame needs repainting.
    func place(origin: CGPoint, area: CGSize) -> Bool {
        guard origin != self.origin || area != self.area else { return false }
        self.origin = origin
        self.area = area
        return true
    }

    func paint(in canvas: CALayer, size: CGSize, seconds: Double) {
        glows.attach(to: canvas)
        let visible = CGRect(origin: origin, size: size)
        EngineBridge.shared.forEachBokehGlow(
            width: area.width, height: area.height, seconds: seconds
        ) { glow in
            let rect = CGRect(origin: glow.center, size: .zero).insetBy(dx: -glow.radius, dy: -glow.radius)
            guard rect.intersects(visible) else { return }
            let style = GlowStyle(red: glow.red, green: glow.green, blue: glow.blue, radius: glow.radius)
            let center = CGPoint(x: glow.center.x - origin.x, y: glow.center.y - origin.y)
            glows.place(style: style, at: center, opacity: glow.alpha) { layer in
                let color = { (alpha: Double) in
                    CGColor(srgbRed: glow.red, green: glow.green, blue: glow.blue, alpha: alpha)
                }
                layer.colors = [color(1), color(Self.discAlphaShare), color(0)]
                layer.bounds = CGRect(origin: .zero, size: rect.size)
            }
        }
        glows.finishFrame()
    }
}
