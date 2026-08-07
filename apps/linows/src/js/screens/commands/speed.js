import { speedTest, localIpv4, copyToClipboard, onWindowShown, onWindowHidden } from '../../ipc.js';
import { globe, eye, eyeOff } from '../../icons.js';
import { prefersReducedMotion, onReducedMotionChange } from '../../platform.js';

// The /speed panel: two counter-rotating comets, download on the outer ring and
// upload on the inner one, around a centre that beats once per round trip.
// Geometry and pacing mirror the macOS SpeedGaugeView, which is the design
// source of truth; the measurement itself is the shared look-netspeed crate.

// Both rings carry the same log scale: one full turn from 0 to 1 Gbps, so a
// slow link and a fibre line are both somewhere readable on the dial.
const ORBIT = {
    maxMbps: 1000,
    bitsPerMegabit: 1_000_000,
    fullTurn: 360,
    // Rotation is measured from straight up, where the scale starts.
    topOfDial: -90,

    // Room outside the download ring for its tick labels.
    labelRoom: 30,
    // The upload ring, as a fraction of the download ring.
    innerRatio: 0.7,
    ringDash: '2 5',
    tickGap: 4,
    majorTickLength: 9,
    minorTickLength: 4,
    labelGap: 21,

    // A comet's tail grows with the rate: a trickle is a short stub, a fast
    // link nearly laps the ring.
    minTailDegrees: 10,
    tailDegreesSpan: 330,
    minTailWidth: 1.5,
    minTailOpacity: 0.18,
    tailOpacitySpan: 0.5,

    // And so does its speed, so the dial reads fast before you read a number.
    minDegreesPerSecond: 10,
    degreesPerSecondSpan: 420,

    // The ping ring travels out and back once per round trip, at a pace scaled
    // from the measured latency.
    pulseSecondsPerMs: 1 / 220,
    minPulseSeconds: 0.35,
    pulseStartRadius: 8,
    pulseOpacity: 0.35,

    // Ticks are labelled at each decade and marked at the 2.5x and 5x steps.
    majorMbps: [1, 10, 100, 1000],
    minorMbps: [2.5, 5, 25, 50, 250, 500],

    // Rate at which a shown value closes the gap to a measured one, per second,
    // applied through exp so the ease is the same on any refresh rate.
    easing: 6,
    // Ceiling on a frame's delta, so a stalled window doesn't fling the comets.
    maxFrameSeconds: 0.05,
};

// Which way a comet runs and how wide its tail grows; the head radii sit on the
// elements themselves in speed.html. Download runs clockwise with its tail
// behind, upload runs the other way.
const DOWNLOAD_STYLE = { tailWidthSpan: 5, trailing: true };
const UPLOAD_STYLE = { tailWidthSpan: 4, trailing: false };

// How recent a reading has to be for opening the panel to reuse it rather than
// spend bandwidth on a fresh run. Anything inside this window reads as "just
// now", so the two read as one decision.
const AUTO_RUN_FRESHNESS_SECS = 60;
// The last reading, so the panel opens with a number instead of a blank.
const STORAGE_KEY_LAST_READING = 'look.speedtest.lastReading';
// Matches core's own word for a phase that measured nothing.
const UNAVAILABLE = 'n/a';
// Stands in for the public address until it is revealed. Fixed width, so the
// line doesn't jump when it is.
const MASKED_ADDRESS = '•••.•••.•••.•••';
// How long a chip reads "copied" before going back to the address.
const COPIED_FEEDBACK_MS = 1400;
const SEPARATOR = '  ·  ';
const ADDRESS_SEPARATOR = '·';
// The dial shows the latency number alone; this carries its unit.
const LATENCY_CAPTION = 'MS LATENCY';
const SVG_NS = 'http://www.w3.org/2000/svg';
const LOG_SPAN = Math.log10(1 + ORBIT.maxMbps);

const LEGEND = [
    { key: 'down', label: 'DOWN', field: 'download_display' },
    { key: 'up', label: 'UP', field: 'upload_display' },
    { key: 'latency', label: 'LATENCY', field: 'latency_display' },
];

// --- State ---
let reading = loadLastReading();
let running = false;
let errorMessage = null;
let runStartedAt = 0;
let localAddress = null;
// The public address starts hidden: it identifies the connection, and this
// panel is a screenshot away from anywhere.
let revealsPublicAddress = false;
let copiedKind = null;
let copiedTimer = null;
let elapsedTimer = null;

// --- Motion ---
let downloadAngle = 0;
let uploadAngle = 0;
let pulseProgress = 0;
let shownDownload = 0;
let shownUpload = 0;
let lastFrame = null;
let frame = null;
let pulseShown = false;
// The launcher hides without leaving the panel, so "panel open" and "window up"
// are separate conditions and syncMotion() needs both.
let windowShown = true;
// Rate positions the comets are currently shaped for; -1 forces a reshape.
let shapedDownload = -1;
let shapedUpload = -1;

// The dial's geometry for the current element size, in CSS pixels. The viewBox
// tracks that box 1:1, so a tick label and a comet head keep the type and stroke
// size macOS draws them at whatever diameter the panel leaves for the dial.
let dial = { width: 0, height: 0, cx: 0, cy: 0, downloadRadius: 0, uploadRadius: 0 };

// --- DOM refs ---
let panel, gaugeEl, scaleEl, pulseEl, downCometEl, upCometEl;
let downTailEl, upTailEl, downHeadEl, upHeadEl;
let latencyEl, captionEl, addressesEl, legendEl, statusEl, readEl, errorEl, carrierEl;
const legendValues = {};

export function init() {
    panel = document.getElementById('cmd-panel-speed');
    gaugeEl = document.getElementById('cmd-speed-gauge');
    scaleEl = document.getElementById('cmd-speed-scale');
    pulseEl = document.getElementById('cmd-speed-pulse');
    downCometEl = document.getElementById('cmd-speed-down-comet');
    upCometEl = document.getElementById('cmd-speed-up-comet');
    downTailEl = document.getElementById('cmd-speed-down-tail');
    upTailEl = document.getElementById('cmd-speed-up-tail');
    downHeadEl = document.getElementById('cmd-speed-down-head');
    upHeadEl = document.getElementById('cmd-speed-up-head');
    latencyEl = document.getElementById('cmd-speed-latency');
    captionEl = document.getElementById('cmd-speed-caption');
    addressesEl = document.getElementById('cmd-speed-addresses');
    legendEl = document.getElementById('cmd-speed-legend');
    statusEl = document.getElementById('cmd-speed-status');
    readEl = document.getElementById('cmd-speed-read');
    errorEl = document.getElementById('cmd-speed-error');
    carrierEl = document.getElementById('cmd-speed-carrier');

    captionEl.textContent = LATENCY_CAPTION;
    buildLegend();

    // Window resizing and the font-size setting both change what the dial has
    // to work with; measureDial reports zero while the panel is hidden.
    new ResizeObserver(() => {
        if (!measureDial()) return;
        buildScale();
        drawMotion();
    }).observe(gaugeEl);

    // The dial is the app's only per-frame loop, and the launcher hides without
    // leaving the panel, so it has to stand down with the window. The OS motion
    // setting can flip mid-session too, including while hidden.
    onWindowHidden(() => {
        windowShown = false;
        syncMotion();
    });
    onWindowShown(() => {
        windowShown = true;
        syncMotion();
    });
    onReducedMotionChange(syncMotion);
}

export function enter() {
    panel.hidden = false;
    measureDial();
    buildScale();
    render();
    syncMotion();
    if (running) startTicking();

    refreshLocalAddress();
    if (!isFresh()) start();
}

export function exit() {
    panel.hidden = true;
    stopMotion();
    // A run outlives the panel, but nothing it would repaint is on screen.
    stopTicking();
    clearCopied();
}

export function handleKey(e) {
    if (e.ctrlKey || e.altKey || e.metaKey) return false;
    if (e.key === 'r' || e.key === 'R') {
        e.preventDefault();
        start();
        return true;
    }
    if (e.key === 'e' || e.key === 'E') {
        e.preventDefault();
        toggleReveal();
        return true;
    }
    return false;
}

function toggleReveal() {
    revealsPublicAddress = !revealsPublicAddress;
    renderAddresses();
}

// --- Running a measurement ---

// The controller outlives the panel, and a run outlives both: the native call
// has no cancel, so closing /speed leaves it measuring and reopening rejoins it
// in progress. Clearing the in-flight flag on close would let a second run
// start alongside the first, and two measurements compete for the bandwidth
// they are each trying to measure.
async function start() {
    if (running) return;
    running = true;
    errorMessage = null;
    runStartedAt = Date.now();
    startTicking();
    render();

    try {
        reading = await speedTest();
        saveLastReading(reading);
    } catch (err) {
        errorMessage = err || 'Speed test failed';
    }

    running = false;
    stopTicking();
    render();
    syncMotion();
}

function isFresh() {
    return reading != null && nowUnix() - reading.measured_at_unix < AUTO_RUN_FRESHNESS_SECS;
}

async function refreshLocalAddress() {
    try {
        localAddress = await localIpv4();
    } catch {
        localAddress = null;
    }
    renderAddresses();
}

// Drives the elapsed counter while the panel watches a run. The count comes off
// `runStartedAt`, so leaving and coming back mid-run rejoins at the right second
// rather than resuming from wherever the timer had got to.
function startTicking() {
    stopTicking();
    elapsedTimer = setInterval(renderStatus, 1000);
}

function stopTicking() {
    if (elapsedTimer) {
        clearInterval(elapsedTimer);
        elapsedTimer = null;
    }
}

function clearCopied() {
    clearTimeout(copiedTimer);
    copiedTimer = null;
    copiedKind = null;
}

function loadLastReading() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY_LAST_READING);
        return raw ? JSON.parse(raw) : null;
    } catch {
        return null;
    }
}

// The public address is dropped on the way to disk: the panel masks it on
// screen, and storage keeping it indefinitely is a stronger retention than that
// implies. A fresh run puts it back.
function saveLastReading(value) {
    try {
        localStorage.setItem(
            STORAGE_KEY_LAST_READING,
            JSON.stringify({ ...value, public_ip: null }),
        );
    } catch {}
}

// --- The dial ---

// Resizes the viewBox to the element's own pixel box and derives the ring radii
// from it, the way the macOS `Dial` derives them from its canvas size. Returns
// whether anything moved, so a resize that changed nothing redraws nothing.
function measureDial() {
    // getBoundingClientRect rather than clientWidth: the latter is not reliable
    // on an SVG element.
    const box = gaugeEl.getBoundingClientRect();
    const width = Math.round(box.width);
    const height = Math.round(box.height);
    if (width < 1 || height < 1) return false;
    if (width === dial.width && height === dial.height) return false;

    const downloadRadius = Math.max(1, Math.min(width, height) / 2 - ORBIT.labelRoom);
    dial = {
        width,
        height,
        cx: width / 2,
        cy: height / 2,
        downloadRadius,
        uploadRadius: downloadRadius * ORBIT.innerRatio,
    };

    gaugeEl.setAttribute('viewBox', `0 0 ${width} ${height}`);
    pulseEl.setAttribute('cx', dial.cx);
    pulseEl.setAttribute('cy', dial.cy);
    placeCentreReadout();
    // The comets are drawn against the old radii until the rate next changes.
    shapedDownload = -1;
    shapedUpload = -1;
    return true;
}

// The latency value and its caption are centred as a pair, the way the macOS
// VStack centres them, so both follow the theme's font size.
function placeCentreReadout() {
    const valueSize = parseFloat(getComputedStyle(latencyEl).fontSize);
    const captionSize = parseFloat(getComputedStyle(captionEl).fontSize);

    latencyEl.setAttribute('x', dial.cx);
    latencyEl.setAttribute('y', dial.cy - captionSize * 0.6);
    captionEl.setAttribute('x', dial.cx);
    captionEl.setAttribute('y', dial.cy + valueSize * 0.42 + captionSize * 0.75);
}

function positionOfMbps(mbps) {
    return Math.min(1, Math.log10(1 + Math.max(0, mbps)) / LOG_SPAN);
}

function positionOfBits(bitsPerSecond) {
    return positionOfMbps(Math.max(0, bitsPerSecond) / ORBIT.bitsPerMegabit);
}

function point(radius, degrees) {
    const radians = ((degrees + ORBIT.topOfDial) * Math.PI) / 180;
    return [dial.cx + radius * Math.cos(radians), dial.cy + radius * Math.sin(radians)];
}

function arcPath(radius, from, to) {
    const [x0, y0] = point(radius, from);
    const [x1, y1] = point(radius, to);
    return `M${x0} ${y0}A${radius} ${radius} 0 ${to - from > 180 ? 1 : 0} 1 ${x1} ${y1}`;
}

function svg(name, attributes) {
    const element = document.createElementNS(SVG_NS, name);
    for (const key in attributes) element.setAttribute(key, attributes[key]);
    return element;
}

// The scale never moves, so it is drawn once per panel open rather than per
// frame alongside the comets. Every part of it takes its colour from CSS, so a
// theme change repaints without touching this.
function buildScale() {
    const fragment = document.createDocumentFragment();
    const outerRadius = dial.downloadRadius + ORBIT.tickGap;

    for (const radius of [dial.downloadRadius, dial.uploadRadius]) {
        fragment.appendChild(
            svg('circle', {
                class: 'cmd-speed-ring',
                cx: dial.cx,
                cy: dial.cy,
                r: radius,
                'stroke-dasharray': ORBIT.ringDash,
            }),
        );
    }

    const tick = (mbps, length, className) => {
        const degrees = positionOfMbps(mbps) * ORBIT.fullTurn;
        const [x1, y1] = point(outerRadius, degrees);
        const [x2, y2] = point(outerRadius + length, degrees);
        fragment.appendChild(svg('line', { class: className, x1, y1, x2, y2 }));
        return degrees;
    };

    for (const mbps of ORBIT.minorMbps) tick(mbps, ORBIT.minorTickLength, 'cmd-speed-tick-minor');

    for (const mbps of ORBIT.majorMbps) {
        const degrees = tick(mbps, ORBIT.majorTickLength, 'cmd-speed-tick-major');
        const [x, y] = point(outerRadius + ORBIT.labelGap, degrees);
        const label = svg('text', {
            class: 'cmd-speed-tick',
            x,
            y,
            'text-anchor': 'middle',
            'dominant-baseline': 'middle',
        });
        label.textContent = mbps >= ORBIT.maxMbps ? '1G' : String(mbps);
        fragment.appendChild(label);
    }

    scaleEl.replaceChildren(fragment);
}

// The one place that decides whether the dial should be moving: the panel has to
// be open, the window up, and the OS not asking for less motion. Under Reduce
// Motion the dial is placed once instead, since nothing will advance the eased
// values. Callers re-run this rather than testing a subset of the three.
function syncMotion() {
    stopMotion();
    lastFrame = null;
    if (panel.hidden || !windowShown) return;
    if (prefersReducedMotion()) {
        snapMotion();
        return;
    }
    frame = requestAnimationFrame(tick);
}

// A paused dial never advances the eased values, so under Reduce Motion a
// landed reading has to be placed directly.
function snapMotion() {
    shownDownload = reading?.download_bits_per_second ?? 0;
    shownUpload = reading?.upload_bits_per_second ?? 0;
    drawMotion();
}

function stopMotion() {
    if (frame) {
        cancelAnimationFrame(frame);
        frame = null;
    }
}

function tick(now) {
    frame = requestAnimationFrame(tick);
    advance(now / 1000);
    drawMotion();
}

function advance(now) {
    const delta = Math.min(ORBIT.maxFrameSeconds, now - (lastFrame ?? now));
    lastFrame = now;
    if (delta <= 0) return;

    shownDownload = eased(shownDownload, reading?.download_bits_per_second ?? 0, delta);
    shownUpload = eased(shownUpload, reading?.upload_bits_per_second ?? 0, delta);

    downloadAngle = (downloadAngle + degreesPerSecond(shownDownload) * delta) % ORBIT.fullTurn;
    uploadAngle =
        (uploadAngle - degreesPerSecond(shownUpload) * delta + ORBIT.fullTurn) % ORBIT.fullTurn;

    const period = Math.max(
        ORBIT.minPulseSeconds,
        (reading?.latency_ms ?? 0) * ORBIT.pulseSecondsPerMs,
    );
    pulseProgress = (pulseProgress + delta / period) % 1;
}

function eased(current, target, delta) {
    return current + (target - current) * (1 - Math.exp(-ORBIT.easing * delta));
}

function degreesPerSecond(bitsPerSecond) {
    return ORBIT.minDegreesPerSecond + positionOfBits(bitsPerSecond) * ORBIT.degreesPerSecondSpan;
}

function drawMotion() {
    drawPulse();
    shapeComets();
    spin(downCometEl, downloadAngle);
    spin(upCometEl, uploadAngle);
}

function spin(cometEl, angle) {
    cometEl.setAttribute('transform', `rotate(${angle} ${dial.cx} ${dial.cy})`);
}

// One expanding ring per round trip: the wait made visible.
function drawPulse() {
    if (reading?.latency_ms == null) {
        // Written once on the way in, not 60 times a second for a hidden ring.
        if (pulseShown) pulseEl.setAttribute('stroke-opacity', 0);
        pulseShown = false;
        return;
    }
    pulseShown = true;
    const outward = pulseProgress < 0.5 ? pulseProgress * 2 : (1 - pulseProgress) * 2;
    pulseEl.setAttribute(
        'r',
        ORBIT.pulseStartRadius + outward * (dial.downloadRadius - ORBIT.pulseStartRadius),
    );
    pulseEl.setAttribute('stroke-opacity', (1 - outward) * ORBIT.pulseOpacity);
}

// A comet's tail and head follow its rate, which only moves while the easing
// converges on a new reading. Rewriting the path every frame would have WebKit
// re-parse it for a shape that hasn't changed.
function shapeComets() {
    const download = positionOfBits(shownDownload);
    const upload = positionOfBits(shownUpload);
    if (download === shapedDownload && upload === shapedUpload) return;
    shapedDownload = download;
    shapedUpload = upload;

    shapeComet(downTailEl, downHeadEl, dial.downloadRadius, download, DOWNLOAD_STYLE);
    shapeComet(upTailEl, upHeadEl, dial.uploadRadius, upload, UPLOAD_STYLE);
}

function shapeComet(tailEl, headEl, radius, position, style) {
    const tailDegrees = ORBIT.minTailDegrees + position * ORBIT.tailDegreesSpan;
    const leading = style.trailing ? -tailDegrees : 0;

    tailEl.setAttribute('d', arcPath(radius, leading, leading + tailDegrees));
    tailEl.setAttribute('stroke-width', ORBIT.minTailWidth + position * style.tailWidthSpan);
    tailEl.setAttribute('stroke-opacity', ORBIT.minTailOpacity + position * ORBIT.tailOpacitySpan);

    const [x, y] = point(radius, 0);
    headEl.setAttribute('cx', x);
    headEl.setAttribute('cy', y);
}

// --- The readouts ---

function render() {
    // The latency colour is the one part of the dial that follows the data;
    // CSS keys the pulse and the centre value off this. See core's latency_level.
    gaugeEl.dataset.latency = reading?.latency_level ?? 'unknown';
    renderAddresses();
    renderLegend();
    renderStatus();
    renderVerdict();
    renderCarrier();
}

// The dial shows the number alone; the caption underneath carries the unit.
function renderLatencyValue() {
    const display = reading?.latency_display ?? UNAVAILABLE;
    latencyEl.textContent = display.split(' ')[0];
}

function buildLegend() {
    legendEl.replaceChildren(
        ...LEGEND.map(({ key, label }) => {
            const entry = document.createElement('span');
            entry.className = 'cmd-speed-legend-entry';

            const dot = document.createElement('span');
            dot.className = `cmd-speed-dot cmd-speed-dot-${key}`;
            entry.appendChild(dot);

            const name = document.createElement('span');
            name.className = 'cmd-speed-legend-label';
            name.textContent = label;
            entry.appendChild(name);

            const value = document.createElement('span');
            value.className = 'cmd-speed-legend-value';
            entry.appendChild(value);
            legendValues[key] = value;

            return entry;
        }),
    );
}

// Dims the standing reading while a fresh one is being measured.
function renderLegend() {
    for (const { key, field } of LEGEND) {
        legendValues[key].textContent = reading?.[field] ?? UNAVAILABLE;
    }
    legendEl.classList.toggle('cmd-speed-superseded', running);
    gaugeEl.classList.toggle('cmd-speed-superseded', running);
    renderLatencyValue();
}

function renderStatus() {
    if (running) {
        statusEl.textContent = `Measuring, ${Math.floor((Date.now() - runStartedAt) / 1000)}s`;
    } else if (!reading) {
        statusEl.textContent = 'Press R to measure';
    } else {
        statusEl.textContent = `Measured ${age(reading)}, press R to run again`;
    }
}

function renderVerdict() {
    const show = reading && !running;
    readEl.textContent = show
        ? `${reading.download_verdict} · latency ${reading.latency_verdict}`
        : '';
    errorEl.textContent = errorMessage ?? '';
    errorEl.hidden = !errorMessage;
}

// Who carries the traffic and roughly where it lands, when the lookup answered.
// Sits under the verdict so the dial keeps the middle.
function renderCarrier() {
    const source = reading?.download_source ? `via ${reading.download_source}` : null;
    carrierEl.textContent = [reading?.provider, reading?.location, source]
        .filter(Boolean)
        .join(SEPARATOR);
}

// Both addresses, since they answer different questions: LAN is what you reach
// this machine on, WAN is what the far end of the test sees.
function renderAddresses() {
    const publicIp = reading?.public_ip;
    const parts = [];

    const icon = document.createElement('span');
    icon.className = 'cmd-speed-address-icon';
    icon.innerHTML = globe;
    parts.push(icon);

    if (!localAddress && !publicIp) {
        parts.push(text('No network'));
    }
    if (localAddress) {
        parts.push(addressChip('LAN', localAddress, localAddress));
    }
    if (publicIp) {
        if (localAddress) parts.push(text(ADDRESS_SEPARATOR));
        parts.push(
            addressChip('WAN', publicIp, revealsPublicAddress ? publicIp : MASKED_ADDRESS),
            revealButton(),
        );
    }

    addressesEl.replaceChildren(...parts);
}

function text(value) {
    const span = document.createElement('span');
    span.textContent = value;
    return span;
}

// One address, click to copy. A masked public address still copies in full:
// hiding it is about what the screen shows, not what you can take with you.
function addressChip(kind, address, shown) {
    const chip = document.createElement('button');
    chip.className = 'cmd-speed-address';
    chip.title = `Copy the ${kind} address`;

    const label = text(`${kind} ${shown}`);
    chip.appendChild(label);

    // The confirmation is an overlay rather than a swapped label, so it cannot
    // shift the line under it.
    if (copiedKind === kind) {
        label.classList.add('cmd-speed-address-masked');
        const copied = text('copied');
        copied.className = 'cmd-speed-copied';
        chip.appendChild(copied);
    }

    chip.addEventListener('click', () => copy(address, kind));
    return chip;
}

function revealButton() {
    const button = document.createElement('button');
    button.className = 'cmd-speed-reveal';
    button.innerHTML = revealsPublicAddress ? eyeOff : eye;
    button.title = revealsPublicAddress
        ? 'Hide the public address (E)'
        : 'Show the public address (E)';
    button.addEventListener('click', toggleReveal);
    return button;
}

// Goes through the app's clipboard command rather than navigator.clipboard, so
// the history monitor knows the write is Look's own and doesn't file the address.
async function copy(address, kind) {
    try {
        await copyToClipboard(address);
    } catch {
        return;
    }
    clearCopied();
    copiedKind = kind;
    renderAddresses();
    copiedTimer = setTimeout(() => {
        copiedKind = null;
        renderAddresses();
    }, COPIED_FEEDBACK_MS);
}

// Anything the panel would still reuse reads as "just now", which is also what
// the relative formatter would otherwise render as "in 0 seconds". Built on
// first use: constructing an Intl formatter is not worth doing on the boot path
// for a panel most sessions never open.
let ageFormatter = null;
// Largest unit first; the 60s guard in age() means 'minute' always matches.
const AGE_UNITS = [
    ['day', 86400],
    ['hour', 3600],
    ['minute', 60],
];

function age(value) {
    const seconds = nowUnix() - value.measured_at_unix;
    if (seconds < AUTO_RUN_FRESHNESS_SECS) return 'just now';

    ageFormatter ??= new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });
    const [unit, size] = AGE_UNITS.find(([, size]) => seconds >= size);
    return ageFormatter.format(-Math.floor(seconds / size), unit);
}

function nowUnix() {
    return Math.floor(Date.now() / 1000);
}
