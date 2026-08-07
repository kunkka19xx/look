import SwiftUI

/// Nonisolated so the drawing helpers, which run inside `Canvas`, can read the
/// same geometry the view lays out from.
private nonisolated enum Orbit {
    /// Both rings carry the same log scale: one full turn from 0 to 1 Gbps, so a
    /// slow link and a fibre line are both somewhere readable on the dial.
    static let maxMbps: Double = 1000
    static let bitsPerMegabit: Double = 1_000_000
    static let fullTurn: Double = 360
    /// Rotation is measured from straight up, where the scale starts.
    static let topOfDial: Double = -90

    /// Room outside the download ring for its tick labels.
    static let labelRoom: CGFloat = 30
    /// The upload ring, as a fraction of the download ring.
    static let innerRatio: CGFloat = 0.7
    static let ringDash: [CGFloat] = [2, 5]
    static let tickGap: CGFloat = 4
    static let majorTickLength: CGFloat = 9
    static let minorTickLength: CGFloat = 4
    static let labelGap: CGFloat = 21
    static let labelSizeDelta: CGFloat = -5

    /// A comet's tail grows with the rate: a trickle is a short stub, a fast
    /// link nearly laps the ring.
    static let minTailDegrees: Double = 10
    static let tailDegreesSpan: Double = 330
    static let minTailWidth: CGFloat = 1.5
    static let minTailOpacity: Double = 0.18
    static let tailOpacitySpan: Double = 0.5

    /// And so does its speed, so the dial reads fast before you read a number.
    static let minDegreesPerSecond: Double = 10
    static let degreesPerSecondSpan: Double = 420

    /// The ping ring travels out and back once per round trip, at a pace scaled
    /// from the measured latency.
    static let pulseSecondsPerMs: Double = 1.0 / 220
    static let minPulseSeconds: Double = 0.35
    static let pulseStartRadius: CGFloat = 8
    static let pulseOpacity: Double = 0.35

    /// Ticks are labelled at each decade and marked at the 2.5x and 5x steps.
    static let majorMbps: [Double] = [1, 10, 100, 1000]
    static let minorMbps: [Double] = [2.5, 5, 25, 50, 250, 500]

    /// Rate at which a shown value closes the gap to a measured one, per second.
    /// Applied through `exp` so the ease is the same on any refresh rate.
    static let easing: Double = 6
    /// Ceiling on a frame's delta, so a stalled window doesn't fling the comets.
    static let maxFrameSeconds: Double = 0.05

    static let centreValueScale: CGFloat = 2.2
    static let centreCaptionTracking: CGFloat = 2

    /// Where a rate sits on the ring, 0 at the top going clockwise. Clamped, so
    /// everything derived from it stops at the end of the scale.
    static func position(ofBitsPerSecond bitsPerSecond: Double) -> Double {
        position(ofMbps: max(0, bitsPerSecond) / bitsPerMegabit)
    }

    static func position(ofMbps mbps: Double) -> Double {
        min(1, log10(1 + max(0, mbps)) / log10(1 + maxMbps))
    }

    static func point(center: CGPoint, radius: CGFloat, degrees: Double) -> CGPoint {
        let radians = (degrees + topOfDial) * .pi / 180
        return CGPoint(
            x: center.x + radius * cos(radians),
            y: center.y + radius * sin(radians)
        )
    }

    static func circle(center: CGPoint, radius: CGFloat) -> Path {
        Path(ellipseIn: CGRect(
            x: center.x - radius,
            y: center.y - radius,
            width: radius * 2,
            height: radius * 2
        ))
    }
}

/// Which ring a comet runs on and how it is drawn. Fixed per direction, so it is
/// resolved once rather than threaded through every frame.
private struct CometStyle {
    let color: Color
    let headRadius: CGFloat
    let tailWidthSpan: CGFloat
    /// Download runs clockwise with its tail behind; upload runs the other way.
    let trailing: Bool
}

/// Theme colours resolved once per body pass. Every `themeStore` accessor walks
/// the preset catalog, which is far too much to repeat ~19 times a frame.
private struct GaugePalette {
    let ring: Color
    let majorTick: Color
    let label: Color
    let download: Color
    let upload: Color
    let latency: Color

    init(themeStore: ThemeStore, latencyLevel: String?) {
        ring = themeStore.dividerColor()
        majorTick = themeStore.secondaryTextColor()
        label = themeStore.mutedTextColor()
        download = themeStore.successColor()
        upload = themeStore.accentColor()
        latency = switch latencyLevel {
        case "bad": themeStore.dangerColor()
        case "warn": themeStore.warningColor()
        case "good": themeStore.successColor()
        default: themeStore.mutedTextColor()
        }
    }
}

/// One frame of the dial's motion. Held in a reference type so advancing it does
/// not invalidate the view: the timeline already drives the redraw.
@MainActor
private final class GaugeMotion {
    var downloadAngle: Double = 0
    var uploadAngle: Double = 0
    var pulseProgress: Double = 0
    var shownDownload: Double = 0
    var shownUpload: Double = 0
    private var lastFrame: Date?

    func advance(to now: Date, download: Double, upload: Double, latencyMs: Double?) {
        let delta = min(Orbit.maxFrameSeconds, now.timeIntervalSince(lastFrame ?? now))
        lastFrame = now
        guard delta > 0 else { return }

        shownDownload = eased(shownDownload, toward: download, delta: delta)
        shownUpload = eased(shownUpload, toward: upload, delta: delta)

        downloadAngle = (downloadAngle + degreesPerSecond(shownDownload) * delta)
            .truncatingRemainder(dividingBy: Orbit.fullTurn)
        uploadAngle = (uploadAngle - degreesPerSecond(shownUpload) * delta + Orbit.fullTurn)
            .truncatingRemainder(dividingBy: Orbit.fullTurn)

        let period = max(Orbit.minPulseSeconds, (latencyMs ?? 0) * Orbit.pulseSecondsPerMs)
        pulseProgress = (pulseProgress + delta / period).truncatingRemainder(dividingBy: 1)
    }

    var isUnstarted: Bool { lastFrame == nil }

    func snap(download: Double, upload: Double) {
        shownDownload = download
        shownUpload = upload
    }

    private func eased(_ current: Double, toward target: Double, delta: Double) -> Double {
        current + (target - current) * (1 - exp(-Orbit.easing * delta))
    }

    private func degreesPerSecond(_ bitsPerSecond: Double) -> Double {
        Orbit.minDegreesPerSecond
            + Orbit.position(ofBitsPerSecond: bitsPerSecond) * Orbit.degreesPerSecondSpan
    }
}

/// Two counter-rotating comets, download on the outer ring and upload on the
/// inner one, around a centre that beats once per round trip. Speed shows as
/// orbit pace and tail length rather than a needle position, so the dial reads
/// as a working instrument even before you read the numbers.
struct SpeedGaugeView: View {
    let downloadBitsPerSecond: Double
    let uploadBitsPerSecond: Double
    let latencyMs: Double?
    /// Preformatted in core, e.g. "26 ms".
    let latencyDisplay: String
    /// `good`, `warn` or `bad`, banded in core beside the words it reads by.
    let latencyLevel: String?
    let themeStore: ThemeStore

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var motion = GaugeMotion()

    var body: some View {
        let palette = GaugePalette(themeStore: themeStore, latencyLevel: latencyLevel)

        return ZStack {
            // The scale never moves, so it is drawn once per size or theme
            // change rather than per frame alongside the comets.
            Canvas { context, size in
                let dial = Dial(size: size)
                context.stroke(
                    Orbit.circle(center: dial.center, radius: dial.downloadRadius),
                    with: .color(palette.ring),
                    style: StrokeStyle(lineWidth: 1, dash: Orbit.ringDash)
                )
                context.stroke(
                    Orbit.circle(center: dial.center, radius: dial.uploadRadius),
                    with: .color(palette.ring),
                    style: StrokeStyle(lineWidth: 1, dash: Orbit.ringDash)
                )
                drawScale(in: &context, dial: dial, palette: palette)
            }

            TimelineView(.animation(paused: reduceMotion)) { timeline in
                Canvas { context, size in
                    motion.advance(
                        to: timeline.date,
                        download: downloadBitsPerSecond,
                        upload: uploadBitsPerSecond,
                        latencyMs: latencyMs
                    )
                    drawMotion(in: &context, dial: Dial(size: size), palette: palette)
                }
            }

            centreReadout(palette: palette)
        }
        .aspectRatio(1, contentMode: .fit)
        .onAppear { snapIfStill() }
        // A paused timeline never advances the eased values, so a reading that
        // lands under Reduce Motion has to be placed directly.
        .onChange(of: downloadBitsPerSecond) { _, _ in snapIfStill() }
        .onChange(of: uploadBitsPerSecond) { _, _ in snapIfStill() }
    }

    private func snapIfStill() {
        guard reduceMotion || motion.isUnstarted else { return }
        motion.snap(download: downloadBitsPerSecond, upload: uploadBitsPerSecond)
    }

    /// The dial's geometry for one canvas size.
    private struct Dial {
        let center: CGPoint
        let downloadRadius: CGFloat
        let uploadRadius: CGFloat

        init(size: CGSize) {
            center = CGPoint(x: size.width / 2, y: size.height / 2)
            downloadRadius = min(size.width, size.height) / 2 - Orbit.labelRoom
            uploadRadius = downloadRadius * Orbit.innerRatio
        }
    }

    // MARK: The scale

    private func drawScale(in context: inout GraphicsContext, dial: Dial, palette: GaugePalette) {
        let outerRadius = dial.downloadRadius + Orbit.tickGap

        for mbps in Orbit.minorMbps {
            drawTick(in: &context, dial: dial, mbps: mbps, length: Orbit.minorTickLength, color: palette.ring)
        }

        for mbps in Orbit.majorMbps {
            drawTick(in: &context, dial: dial, mbps: mbps, length: Orbit.majorTickLength, color: palette.majorTick)
            context.draw(
                Text(Self.tickLabel(mbps)).font(labelFont).foregroundStyle(palette.label),
                at: Orbit.point(
                    center: dial.center,
                    radius: outerRadius + Orbit.labelGap,
                    degrees: Orbit.position(ofMbps: mbps) * Orbit.fullTurn
                )
            )
        }
    }

    private func drawTick(
        in context: inout GraphicsContext,
        dial: Dial,
        mbps: Double,
        length: CGFloat,
        color: Color
    ) {
        let degrees = Orbit.position(ofMbps: mbps) * Orbit.fullTurn
        let outerRadius = dial.downloadRadius + Orbit.tickGap

        var tick = Path()
        tick.move(to: Orbit.point(center: dial.center, radius: outerRadius, degrees: degrees))
        tick.addLine(to: Orbit.point(center: dial.center, radius: outerRadius + length, degrees: degrees))
        context.stroke(tick, with: .color(color), lineWidth: length == Orbit.majorTickLength ? 1.5 : 1)
    }

    // MARK: The moving parts

    private func drawMotion(in context: inout GraphicsContext, dial: Dial, palette: GaugePalette) {
        drawPingPulse(in: &context, dial: dial, palette: palette)

        drawComet(
            in: &context,
            center: dial.center,
            radius: dial.downloadRadius,
            angle: motion.downloadAngle,
            rate: motion.shownDownload,
            style: CometStyle(
                color: palette.download,
                headRadius: 5.5,
                tailWidthSpan: 5,
                trailing: true
            )
        )

        drawComet(
            in: &context,
            center: dial.center,
            radius: dial.uploadRadius,
            angle: motion.uploadAngle,
            rate: motion.shownUpload,
            style: CometStyle(
                color: palette.upload,
                headRadius: 4.5,
                tailWidthSpan: 4,
                trailing: false
            )
        )
    }

    /// One expanding ring per round trip: the wait made visible.
    private func drawPingPulse(in context: inout GraphicsContext, dial: Dial, palette: GaugePalette) {
        guard latencyMs != nil else { return }

        let progress = motion.pulseProgress
        let outward = progress < 0.5 ? progress * 2 : (1 - progress) * 2
        let radius = Orbit.pulseStartRadius + outward * (dial.downloadRadius - Orbit.pulseStartRadius)
        context.stroke(
            Orbit.circle(center: dial.center, radius: radius),
            with: .color(palette.latency.opacity((1 - outward) * Orbit.pulseOpacity)),
            lineWidth: 1
        )
    }

    private func drawComet(
        in context: inout GraphicsContext,
        center: CGPoint,
        radius: CGFloat,
        angle: Double,
        rate: Double,
        style: CometStyle
    ) {
        let position = Orbit.position(ofBitsPerSecond: rate)
        let tailDegrees = Orbit.minTailDegrees + position * Orbit.tailDegreesSpan
        let leading = style.trailing ? angle - tailDegrees : angle

        var tail = Path()
        tail.addArc(
            center: center,
            radius: radius,
            startAngle: .degrees(leading + Orbit.topOfDial),
            endAngle: .degrees(leading + tailDegrees + Orbit.topOfDial),
            clockwise: false
        )
        context.stroke(
            tail,
            with: .color(style.color.opacity(Orbit.minTailOpacity + position * Orbit.tailOpacitySpan)),
            style: StrokeStyle(
                lineWidth: Orbit.minTailWidth + CGFloat(position) * style.tailWidthSpan,
                lineCap: .round
            )
        )

        context.fill(
            Orbit.circle(
                center: Orbit.point(center: center, radius: radius, degrees: angle),
                radius: style.headRadius
            ),
            with: .color(style.color)
        )
    }

    private func centreReadout(palette: GaugePalette) -> some View {
        VStack(spacing: 0) {
            Text(Self.latencyValue(latencyDisplay))
                .font(.system(
                    size: CGFloat(themeStore.settings.fontSize) * Orbit.centreValueScale,
                    weight: .medium,
                    design: .monospaced
                ))
                .foregroundStyle(palette.latency)

            Text("MS LATENCY")
                .font(labelFont)
                .foregroundStyle(palette.label)
                .tracking(Orbit.centreCaptionTracking)
        }
    }

    private var labelFont: Font {
        .system(
            size: CGFloat(themeStore.settings.fontSize) + Orbit.labelSizeDelta,
            design: .monospaced
        )
    }

    private static func tickLabel(_ mbps: Double) -> String {
        mbps >= Orbit.maxMbps ? "1G" : "\(Int(mbps))"
    }

    /// The dial shows the number alone; the caption underneath carries the unit.
    private static func latencyValue(_ display: String) -> String {
        display.split(separator: " ").first.map(String.init) ?? display
    }
}
