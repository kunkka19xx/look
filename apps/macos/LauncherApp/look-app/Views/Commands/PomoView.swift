import AppKit
import Combine
import Observation
import SwiftUI
import UniformTypeIdentifiers

// ── Shared state ──────────────────────────────────────────────────────
//
// PomoState is the single source of truth for an active pomodoro across
// the launcher. PomoView reads/writes it; the menu bar item observes it
// for the always-visible mini-timer. Lives as a @StateObject inside
// PomoView (not app-global) - closing the launcher window keeps the
// timer running because the SwiftUI WindowGroup retains its view tree
// while the window is hidden.

@Observable
final class PomoState {
    var sessions: [PomoSession] = PomoCommand.defaultSessions()
    var timerStyle: PomoTimerStyle = .modern
    let music = PomoMusicPlayer()

    private(set) var activeIndex: Int? = nil      // nil = no session selected
    private(set) var secondsLeft: Int = 0
    private(set) var running: Bool = false

    // UI state for the idle fade - kept here (not as @State on PomoView)
    // so other views (the launcher's command sidebar) can react to it
    // and collapse out of the way.
    var idle: Bool = false

    @ObservationIgnored private var cancellable: AnyCancellable?
    @ObservationIgnored private var lastTickAt: Date?
    @ObservationIgnored private var didNotifyEndingSoon = false

    init() {
        let snap = PomoPersistence.load()
        sessions = snap.sessions
        timerStyle = snap.timerStyle
        music.restore(folderPath: snap.musicFolderPath)
    }

    // ── Computed ────────────────────────────────────────────────────────

    var currentSession: PomoSession? {
        guard let i = activeIndex, sessions.indices.contains(i) else { return nil }
        return sessions[i]
    }

    var totalForCurrentPhase: Int {
        guard let s = currentSession else { return 0 }
        return s.durationMinutes * 60
    }

    var progress: Double {
        let total = totalForCurrentPhase
        guard total > 0 else { return 0 }
        return 1.0 - Double(secondsLeft) / Double(total)
    }

    // ── Controls ────────────────────────────────────────────────────────

    func toggle() {
        if activeIndex == nil {
            guard let first = sessions.first else { return }
            activeIndex = 0
            secondsLeft = first.durationMinutes * 60
            didNotifyEndingSoon = false
            startTicking()
            running = true
        } else {
            running.toggle()
            if running { startTicking() } else { stopTicking() }
        }
    }

    func reset() {
        stopTicking()
        running = false
        activeIndex = nil
        secondsLeft = 0
        didNotifyEndingSoon = false
    }

    // Removes a session by index and keeps `activeIndex` valid.
    // - Removing the currently-running session stops the timer.
    // - Removing a session before the active index shifts it down by one.
    // - Removing a session after the active index leaves it alone.
    func removeSession(at idx: Int) {
        guard sessions.indices.contains(idx) else { return }
        if let active = activeIndex {
            if idx == active {
                stopTicking()
                running = false
                activeIndex = nil
                secondsLeft = 0
            } else if idx < active {
                activeIndex = active - 1
            }
        }
        sessions.remove(at: idx)
    }

    func skip() {
        guard let i = activeIndex else { return }
        advance(from: i)
    }

    // ── Persistence side-effects ────────────────────────────────────────

    func persist() {
        PomoPersistence.save(.init(
            sessions: sessions,
            timerStyle: timerStyle,
            musicFolderPath: music.folderPath
        ))
    }

    // ── Tick driver ─────────────────────────────────────────────────────
    //
    // Wall-clock based: we record `lastTickAt` and on each fire we deduct
    // the elapsed seconds. This survives sleep/wake correctly - closing
    // a laptop for 5 min returns with 5 fewer minutes left, not paused.

    private func startTicking() {
        stopTicking()
        lastTickAt = Date()
        cancellable = Timer.publish(every: 0.5, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] now in self?.tick(now: now) }
    }

    private func stopTicking() {
        cancellable?.cancel()
        cancellable = nil
        lastTickAt = nil
    }

    private func tick(now: Date) {
        guard running, let lastTickAt else { return }
        let elapsed = Int(now.timeIntervalSince(lastTickAt))
        guard elapsed >= 1 else { return }
        self.lastTickAt = lastTickAt.addingTimeInterval(TimeInterval(elapsed))

        secondsLeft -= elapsed
        // Fire the "ending soon" notification once per phase, when
        // remaining time first crosses below the threshold.
        if !didNotifyEndingSoon,
           secondsLeft <= PomoCommand.endingSoonThresholdSeconds,
           secondsLeft > 0,
           let s = currentSession
        {
            didNotifyEndingSoon = true
            PomoNotifications.notifyEndingSoon(session: s, secondsLeft: secondsLeft)
        }
        if secondsLeft <= 0 {
            secondsLeft = 0
            handlePhaseEnd()
        }
    }

    private func handlePhaseEnd() {
        guard let i = activeIndex else { return }
        _ = sessions[i]
        didNotifyEndingSoon = false  // ready to fire again for the next phase

        // Only the "ending soon" notification (10s before end) is shown
        // to the user - the phase-end moment itself is silent because
        // the timer / phase color visibly transitioning is enough.
        let next: PomoSession? = (i + 1 < sessions.count) ? sessions[i + 1] : nil

        if next != nil {
            activeIndex = i + 1
            secondsLeft = sessions[i + 1].durationMinutes * 60
            // Auto-continue to keep flow moving; user can pause if needed.
            running = true
            lastTickAt = Date()
        } else {
            stopTicking()
            running = false
            activeIndex = nil
        }
    }

    private func advance(from i: Int) {
        // Reset the per-phase notification flag so the next session's
        // own ending-soon alert can fire - covers both the natural
        // phase-end path and `skip()`, which would otherwise carry the
        // already-notified flag into the next session.
        didNotifyEndingSoon = false

        let next = i + 1
        if sessions.indices.contains(next) {
            activeIndex = next
            secondsLeft = sessions[next].durationMinutes * 60
            lastTickAt = Date()
            if running { /* keep running */ }
        } else {
            stopTicking()
            running = false
            activeIndex = nil
            secondsLeft = 0
        }
    }
}

// ── Menu bar bridge ───────────────────────────────────────────────────
//
// AppDelegate creates a global PomoState reference at launch (so the
// menu bar can show the timer even when the launcher window is closed)
// and passes it into the PomoView via the environment. We avoid a
// singleton - PomoState is shared but not global.

enum PomoSharedState {
    @MainActor static let shared = PomoState()
}

// ── The view ──────────────────────────────────────────────────────────

struct PomoView: View {
    let themeStore: ThemeStore
    @Bindable var state: PomoState

    @State private var showSessionList = false
    @State private var showSettings = false
    @State private var idleResetToken: UUID = UUID()
    private var idle: Bool { state.idle }

    init(themeStore: ThemeStore, state: PomoState? = nil) {
        self.themeStore = themeStore
        self.state = state ?? PomoSharedState.shared
    }

    var body: some View {
        VStack(spacing: 8) {
            headerBar
                .opacity(idle ? 0 : 1)
                .animation(.easeInOut(duration: 0.4), value: idle)

            GeometryReader { geo in
                ScrollView(.vertical, showsIndicators: false) {
                    VStack(spacing: 8) {
                        VStack(spacing: 8) {
                            timerCard
                            controlsRow
                                .opacity(idle ? 0 : 1)
                                .animation(.easeInOut(duration: 0.4), value: idle)
                                // Defensive: nothing inside the controls
                                // row should animate. Suppresses any
                                // inherited animation transaction so the
                                // button color flip on Start/Pause is an
                                // instant change, not an interpolated
                                // smear.
                                .transaction { $0.animation = nil }
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)

                        sessionListToggleAndList
                            .opacity(idle ? 0 : 1)
                            .animation(.easeInOut(duration: 0.4), value: idle)
                    }
                    .frame(minHeight: geo.size.height)
                }
            }

            musicCard
        }
        .padding(10)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .onAppear {
            scheduleIdleReset()
        }
        .onDisappear {
            state.persist()
        }
        .onChange(of: state.running) { _, _ in
            restoreFromIdle()
            scheduleIdleReset()
        }
        .onChange(of: state.activeIndex) { _, _ in
            // Don't restore from standby on auto-advance - the user
            // explicitly wants to stay in focus mode across the phase
            // boundary. Re-arming the idle countdown is also unnecessary
            // here; the existing schedule (or the next activity event)
            // will handle it.
        }
        .onChange(of: state.sessions) { _, _ in
            state.persist()
        }
        .onChange(of: state.timerStyle) { _, _ in
            state.persist()
        }
        .onChange(of: state.music.folderPath) { _, _ in
            state.persist()
        }
        .background(KeyCommandsRecognizer(
            onToggle: { state.toggle() },
            onR: { state.reset() },
            onP: { state.music.togglePlay() },
            onActivity: { restoreFromIdle() }
        ))
    }

    private func restoreFromIdle() {
        // Plain assignment, no `withAnimation`. The fade is driven by
        // `.animation(_:value: idle)` modifiers on the views that
        // actually fade - so an in-progress idle change can't capture
        // unrelated state mutations (e.g. a button color flipping when
        // the user clicks Start/Pause) and animate them too. That was
        // showing up as a yellow/red halo around the buttons.
        if state.idle { state.idle = false }
        // Always re-arm the 5s countdown - without this, an activity
        // event silently consumed the schedule and standby never
        // re-triggered. Token logic in scheduleIdleReset handles the
        // pile-up of pending tasks (only the latest one fires).
        scheduleIdleReset()
    }

    // ── Header ──────────────────────────────────────────────────────────

    private var headerBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "timer")
                .foregroundStyle(themeStore.accentColor())
            Text(state.activeIndex.map { "Running: \(state.sessions[$0].name)" } ?? "\(state.sessions.count) sessions planned")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
                .lineLimit(1)
            Spacer(minLength: 4)
            Text("/pomo")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
                .padding(.horizontal, 8)
                .padding(.vertical, 2)
                .background(themeStore.selectionFillColor(), in: Capsule())
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(themeStore.commandModePanelColor(), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    // ── Timer + per-session label ──────────────────────────────────────

    private var timerCard: some View {
        // HStack with leading + trailing Spacer to center the clock
        // horizontally regardless of card width. Clock sits in the middle
        // of the panel column rather than top-left.
        VStack(spacing: 12) {
            if let s = state.currentSession, !idle {
                Text(s.name)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
            HStack {
                Spacer(minLength: 0)
                // Isolated subview so the per-second `secondsLeft` / `progress`
                // changes only invalidate the timer, not the entire panel.
                PomoTimerArea(
                    state: state,
                    themeStore: themeStore,
                    size: idle ? 240 : 180
                )
                Spacer(minLength: 0)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(14)
        .background(themeStore.commandModePanelColor(), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    // ── Controls ───────────────────────────────────────────────────────

    private var controlsRow: some View {
        HStack(spacing: 8) {
            Spacer(minLength: 0)

            Button {
                state.toggle()
            } label: {
                Text(toggleLabel)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                    .padding(.horizontal, 14).padding(.vertical, 6)
                    .background(toggleColor, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                    .foregroundStyle(themeStore.onAccentColor())
            }
            .buttonStyle(.plain)

            if state.activeIndex != nil {
                Button {
                    state.skip()
                } label: {
                    Text("Skip ▸")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                        .padding(.horizontal, 12).padding(.vertical, 6)
                        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                        .foregroundStyle(themeStore.secondaryTextColor())
                }
                .buttonStyle(.plain)

                Button {
                    state.reset()
                } label: {
                    Text("Reset (R)")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                        .padding(.horizontal, 12).padding(.vertical, 6)
                        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                        .foregroundStyle(themeStore.dangerColor())
                }
                .buttonStyle(.plain)
            }

            Button {
                showSettings.toggle()
            } label: {
                Image(systemName: "gearshape")
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
            .buttonStyle(.plain)

            Spacer(minLength: 0)
        }
    }

    private var toggleLabel: String {
        if state.activeIndex == nil { return "Start (Space)" }
        return state.running ? "Pause (Space)" : "Resume (Space)"
    }

    private var toggleColor: Color {
        if state.activeIndex == nil { return themeStore.accentColor() }
        return state.running ? themeStore.warningColor() : themeStore.successColor()
    }

    // ── Session list ───────────────────────────────────────────────────

    private var sessionListToggleAndList: some View {
        VStack(alignment: .leading, spacing: 6) {
            // Settings panel (timer style picker) appears ABOVE the
            // session-list toggle when its gear is on - placing it below
            // the toggle was confusing, since it visually buried the
            // setting underneath the unrelated list section.
            if showSettings {
                styleSettingsCard
            }

            HStack(spacing: 6) {
                Button {
                    showSessionList.toggle()
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: showSessionList ? "chevron.down" : "chevron.right")
                        Text("Session List (\(state.sessions.count))")
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                    }
                    .padding(.horizontal, 10).padding(.vertical, 5)
                    .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                    .foregroundStyle(showSessionList ? themeStore.accentColor() : themeStore.secondaryTextColor())
                }
                .buttonStyle(.plain)

                Spacer()

                // Add buttons only when the list is open - they wouldn't
                // make sense (or be visible) when the list is collapsed.
                if showSessionList {
                    Button {
                        state.sessions.append(PomoSession(type: .focus, durationMinutes: PomoCommand.focusDefaultMinutes, name: "Focus"))
                    } label: {
                        Text("+ Focus")
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                            .padding(.horizontal, 10).padding(.vertical, 4)
                            .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                            .foregroundStyle(themeStore.dangerColor())
                    }
                    .buttonStyle(.plain)

                    Button {
                        state.sessions.append(PomoSession(type: .break, durationMinutes: PomoCommand.breakDefaultMinutes, name: "Break"))
                    } label: {
                        Text("+ Break")
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                            .padding(.horizontal, 10).padding(.vertical, 4)
                            .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                            .foregroundStyle(themeStore.successColor())
                    }
                    .buttonStyle(.plain)
                }
            }

            if showSessionList {
                sessionListEditor
            }
        }
    }

    private var styleSettingsCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Timer style")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                .foregroundStyle(themeStore.mutedTextColor())
            HStack(spacing: 6) {
                ForEach(PomoTimerStyle.allCases) { style in
                    Button {
                        state.timerStyle = style
                    } label: {
                        Text(style.title)
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                            .padding(.horizontal, 10).padding(.vertical, 4)
                            .background(state.timerStyle == style ? themeStore.accentColor() : themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                            .foregroundStyle(state.timerStyle == style ? themeStore.onAccentColor() : themeStore.secondaryTextColor())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .padding(10)
        .background(themeStore.commandModePanelColor(), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var sessionListEditor: some View {
        VStack(spacing: 4) {
            ForEach(Array(state.sessions.enumerated()), id: \.element.id) { idx, session in
                SessionRow(
                    session: binding(for: session.id),
                    isActive: idx == state.activeIndex,
                    isPast: state.activeIndex.map { idx < $0 } ?? false,
                    onDelete: {
                        if let removeIdx = state.sessions.firstIndex(where: { $0.id == session.id }) {
                            state.removeSession(at: removeIdx)
                        }
                    },
                    themeStore: themeStore
                )
            }
        }
    }

    // ID-based binding so that mutating one session never depends on a
    // stale captured index - important during ForEach re-render after a
    // delete.
    private func binding(for id: PomoSession.ID) -> Binding<PomoSession> {
        Binding(
            get: { state.sessions.first(where: { $0.id == id })
                ?? PomoSession(type: .focus, durationMinutes: 1, name: "") },
            set: { newValue in
                if let idx = state.sessions.firstIndex(where: { $0.id == id }) {
                    state.sessions[idx] = newValue
                }
            }
        )
    }

    // ── Music (stub) ───────────────────────────────────────────────────

    private var musicCard: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "music.note")
                    .foregroundStyle(themeStore.accentColor())
                Text(musicTrackTitle)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                    .foregroundStyle(themeStore.fontColor())
                    .lineLimit(1)
                Spacer(minLength: 0)
                musicButton(systemName: "backward.fill") { state.music.prev() }
                // Single play/pause button - icon swaps on state.
                musicButton(systemName: state.music.isPlaying ? "pause.fill" : "play.fill") {
                    state.music.togglePlay()
                }
                musicButton(systemName: "forward.fill") { state.music.next() }
            }
            HStack(spacing: 6) {
                Image(systemName: "folder")
                    .foregroundStyle(themeStore.mutedTextColor())
                Text(state.music.folderPath ?? "No folder selected")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer(minLength: 0)
                Button("Choose…") {
                    pickMusicFolder()
                }
                .buttonStyle(.plain)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                .foregroundStyle(themeStore.accentColor())

                if state.music.hasFolder {
                    Button("Clear") {
                        state.music.clearFolder()
                    }
                    .buttonStyle(.plain)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                    .foregroundStyle(themeStore.dangerColor())
                }
            }
        }
        .padding(10)
        .background(themeStore.commandModePanelColor(), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var musicTrackTitle: String {
        if !state.music.hasFolder { return "Pick a folder to enable music" }
        if state.music.tracks.isEmpty { return "(no audio files)" }
        return state.music.currentTitle ?? "(press play)"
    }

    private func musicButton(systemName: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .foregroundStyle(themeStore.secondaryTextColor())
        }
        .buttonStyle(.plain)
    }

    private func pickMusicFolder() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        if panel.runModal() == .OK, let url = panel.url {
            state.music.setFolder(url)
        }
    }

    // ── Idle fade ──────────────────────────────────────────────────────

    private func scheduleIdleReset() {
        idleResetToken = UUID()
        let token = idleResetToken
        guard state.running else { state.idle = false; return }
        DispatchQueue.main.asyncAfter(deadline: .now() + PomoCommand.idleFadeSeconds) {
            guard token == idleResetToken, state.running else { return }
            state.idle = true
        }
    }
}

// ── Timer area: the only subview that should re-render every tick ────

private struct PomoTimerArea: View {
    let state: PomoState
    let themeStore: ThemeStore
    let size: CGFloat

    var body: some View {
        let color = phaseColor(for: state.currentSession?.type, themeStore: themeStore)
        let dim = themeStore.controlFillColor()
        let text = themeStore.fontColor()
        switch state.timerStyle {
        case .modern:
            ModernRingTimer(progress: state.progress, secondsLeft: state.secondsLeft, color: color, dimColor: dim, textColor: text, size: size)
        case .vintage:
            VintageDialTimer(progress: state.progress, secondsLeft: state.secondsLeft, color: color, dimColor: dim, textColor: text, size: size)
        case .minimal:
            MinimalTextTimer(progress: state.progress, secondsLeft: state.secondsLeft, color: color, dimColor: dim, textColor: text, size: size)
        }
    }
}

// ── Phase color helper ─────────────────────────────────────────────────

private func phaseColor(for type: PomoSession.SessionType?, themeStore: ThemeStore) -> Color {
    switch type {
    case .focus: return themeStore.dangerColor()
    case .break: return themeStore.successColor()
    case nil: return themeStore.accentColor()
    }
}

// ── Session row editor ─────────────────────────────────────────────────

private struct SessionRow: View {
    @Binding var session: PomoSession
    let isActive: Bool
    let isPast: Bool
    let onDelete: () -> Void
    let themeStore: ThemeStore

    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(session.type == .focus ? themeStore.dangerColor() : themeStore.successColor())
                .frame(width: 8, height: 8)

            TextField("Name", text: $session.name)
                .textFieldStyle(.plain)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: isActive ? .semibold : .regular))
                .foregroundStyle(themeStore.fontColor())

            TextField("", value: $session.durationMinutes, format: .number)
                .textFieldStyle(.plain)
                .frame(width: 36)
                .multilineTextAlignment(.center)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.fontColor())
                .padding(.horizontal, 4).padding(.vertical, 2)
                .background(themeStore.panelFillColor(), in: RoundedRectangle(cornerRadius: 4, style: .continuous))

            Text("m")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())

            Button(session.type == .focus ? "F" : "B") {
                session.type = session.type == .focus ? .break : .focus
            }
            .buttonStyle(.plain)
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
            .foregroundStyle(session.type == .focus ? themeStore.dangerColor() : themeStore.successColor())

            Button { onDelete() } label: {
                Image(systemName: "xmark")
                    .foregroundStyle(themeStore.dangerColor())
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 6).padding(.vertical, 4)
        .background(isActive ? themeStore.selectionFillColor() : Color.clear, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
        .opacity(isPast ? 0.45 : 1.0)
    }
}

// ── Local key recognizer for Space + R + P ─────────────────────────────
//
// We don't want these to fight with text fields, so the recognizer
// installs a local monitor only while the view is on screen and ignores
// events whose first responder is an editable NSTextView/NSTextField.

private struct KeyCommandsRecognizer: NSViewRepresentable {
    var onToggle: () -> Void
    var onR: () -> Void
    var onP: () -> Void
    var onActivity: () -> Void

    func makeNSView(context: Context) -> KeyCommandsHostView {
        let v = KeyCommandsHostView()
        v.onToggle = onToggle
        v.onR = onR
        v.onP = onP
        v.onActivity = onActivity
        return v
    }

    func updateNSView(_ nsView: KeyCommandsHostView, context: Context) {
        nsView.onToggle = onToggle
        nsView.onR = onR
        nsView.onP = onP
        nsView.onActivity = onActivity
    }
}

private final class KeyCommandsHostView: NSView {
    var onToggle: (() -> Void)?
    var onR: (() -> Void)?
    var onP: (() -> Void)?
    var onActivity: (() -> Void)?
    nonisolated(unsafe) private var keyMonitor: Any?
    nonisolated(unsafe) private var activityMonitor: Any?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil { installMonitors() } else { removeMonitors() }
    }

    deinit {
        // Inline cleanup so deinit stays nonisolated.
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        if let activityMonitor { NSEvent.removeMonitor(activityMonitor) }
    }

    private func installMonitors() {
        if keyMonitor == nil {
            keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                guard let self else { return event }
                self.onActivity?()
                // Don't steal keystrokes from text editing.
                if event.window?.firstResponder is NSText { return event }
                if let text = event.window?.firstResponder as? NSView,
                   text.isKind(of: NSTextField.self) || text.isKind(of: NSTextView.self) { return event }
                let chars = event.charactersIgnoringModifiers?.lowercased() ?? ""
                let plainModifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask).isEmpty
                // Space (49), no modifiers - start/pause. Cmd+Space
                // (the launcher hotkey) carries .command and is rejected
                // by the plainModifiers check, so it passes through.
                if event.keyCode == 49 && plainModifiers {
                    self.onToggle?()
                    return nil
                }
                if chars == "r" && plainModifiers {
                    self.onR?()
                    return nil
                }
                if chars == "p" && plainModifiers {
                    self.onP?()
                    return nil
                }
                return event
            }
        }
        // Restore-from-idle on any pointer interaction. Mouse-moved
        // events are deliberately skipped - they fire constantly and
        // would defeat the idle fade entirely.
        if activityMonitor == nil {
            let mask: NSEvent.EventTypeMask = [.leftMouseDown, .rightMouseDown, .otherMouseDown, .scrollWheel, .flagsChanged]
            activityMonitor = NSEvent.addLocalMonitorForEvents(matching: mask) { [weak self] event in
                self?.onActivity?()
                return event
            }
        }
    }

    private func removeMonitors() {
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        if let activityMonitor { NSEvent.removeMonitor(activityMonitor) }
        keyMonitor = nil
        activityMonitor = nil
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Timer renderers
// ═══════════════════════════════════════════════════════════════════════

private struct ModernRingTimer: View {
    let progress: Double
    let secondsLeft: Int
    let color: Color
    let dimColor: Color
    let textColor: Color
    let size: CGFloat

    var body: some View {
        ZStack {
            Circle()
                .stroke(dimColor, lineWidth: 6)
            // No `.animation(_:value: progress)` - the trim was animating
            // every tick, and that animation transaction was leaking out
            // and rendering a yellow/red halo around the controls below.
            // We trade smooth interpolation for visual stability; the
            // ring still updates once per tick.
            Circle()
                .trim(from: 0, to: max(0, min(1, progress)))
                .stroke(color, style: StrokeStyle(lineWidth: 6, lineCap: .round))
                .rotationEffect(.degrees(-90))
            Text(PomoCommand.formattedRemaining(secondsLeft))
                .font(.system(size: size * 0.22, weight: .bold, design: .monospaced))
                .foregroundStyle(textColor)
        }
        .frame(width: size, height: size)
    }
}

private struct VintageDialTimer: View {
    let progress: Double
    let secondsLeft: Int
    let color: Color
    let dimColor: Color
    let textColor: Color
    let size: CGFloat

    var body: some View {
        // Vintage style is purely analog - the dial face conveys the
        // remaining time via the needle position. No digital readout.
        Canvas { ctx, canvasSize in
            drawDial(in: ctx, canvasSize: canvasSize)
        }
        .frame(width: size, height: size)
    }

    private func drawDial(in ctx: GraphicsContext, canvasSize: CGSize) {
        let center = CGPoint(x: canvasSize.width / 2, y: canvasSize.height / 2)
        let r = (min(canvasSize.width, canvasSize.height) - 20) / 2
        let ticks = 60

        for i in 0..<ticks {
            drawTick(in: ctx, center: center, radius: r, index: i, ticks: ticks)
        }
        drawNeedle(in: ctx, center: center, radius: r)
        let hub = CGRect(x: center.x - 4, y: center.y - 4, width: 8, height: 8)
        ctx.fill(Path(ellipseIn: hub), with: .color(color))
    }

    private func drawTick(in ctx: GraphicsContext, center: CGPoint, radius r: CGFloat, index i: Int, ticks: Int) {
        let angle: Double = Double(i) / Double(ticks) * 2 * .pi - .pi / 2
        let isFive = i % 5 == 0
        let innerR: CGFloat = r - (isFive ? 12 : 6)
        let outerR: CGFloat = r - 2
        let cosA = CGFloat(cos(angle))
        let sinA = CGFloat(sin(angle))
        let p1 = CGPoint(x: center.x + innerR * cosA, y: center.y + innerR * sinA)
        let p2 = CGPoint(x: center.x + outerR * cosA, y: center.y + outerR * sinA)
        let filled = Double(i) / Double(ticks) <= progress
        var path = Path()
        path.move(to: p1)
        path.addLine(to: p2)
        let style = StrokeStyle(lineWidth: isFive ? 2.5 : 1.2, lineCap: .round)
        // Inactive tick = derived from the theme text color (which is
        // bright in dark themes), at moderate opacity. Reads as a clear
        // tick mark instead of disappearing into the dark backdrop.
        let inactiveTick = textColor.opacity(0.42)
        ctx.stroke(path, with: .color(filled ? color : inactiveTick), style: style)
    }

    private func drawNeedle(in ctx: GraphicsContext, center: CGPoint, radius r: CGFloat) {
        let angle: Double = progress * 2 * .pi - .pi / 2
        let needleR: CGFloat = r - 22
        let endX = center.x + needleR * CGFloat(cos(angle))
        let endY = center.y + needleR * CGFloat(sin(angle))
        var needle = Path()
        needle.move(to: center)
        needle.addLine(to: CGPoint(x: endX, y: endY))
        let style = StrokeStyle(lineWidth: 2.5, lineCap: .round)
        ctx.stroke(needle, with: .color(color), style: style)
    }
}

private struct MinimalTextTimer: View {
    let progress: Double
    let secondsLeft: Int
    let color: Color
    let dimColor: Color
    let textColor: Color
    let size: CGFloat

    var body: some View {
        VStack(spacing: 12) {
            Text(PomoCommand.formattedRemaining(secondsLeft))
                .font(.system(size: size * 0.32, weight: .heavy, design: .monospaced))
                .foregroundStyle(textColor)

            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(dimColor)
                    // No `.animation(_:value: progress)` - see ModernRingTimer.
                    Capsule()
                        .fill(color)
                        .frame(width: geo.size.width * max(0, min(1, progress)))
                }
            }
            .frame(width: size * 0.8, height: 4)
        }
        .frame(width: size, height: size)
    }
}
