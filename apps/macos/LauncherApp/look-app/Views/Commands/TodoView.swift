import AppKit
import SwiftUI

// The panel owns its full chrome (top search bar, button bar, page
// toggle, hint bar) the same way PomoView owns its header, so /todo is
// marked as not accepting the outer command input (see
// activeCommandAcceptsInput).

enum TodoPage {
    case tasks
    case analytics
}

struct TodoView: View {
    let themeStore: ThemeStore
    @Bindable var state: TodoState

    @State private var page: TodoPage
    @State private var search = ""
    @State private var savedToast = false
    @State private var savedToastToken = UUID()
    @FocusState private var searchFocused: Bool

    init(themeStore: ThemeStore, state: TodoState? = nil, initialPage: TodoPage = .tasks) {
        self.themeStore = themeStore
        self.state = state ?? TodoSharedState.shared
        _page = State(initialValue: initialPage)
    }

    var body: some View {
        VStack(spacing: 8) {
            topInputBar

            if page == .tasks {
                buttonBar
            }

            Group {
                if page == .tasks {
                    TodoTasksPage(themeStore: themeStore, state: state, search: search)
                } else {
                    TodoAnalyticsPage(themeStore: themeStore)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        // The launcher's global hint bar already covers /todo's shortcuts,
        // so the panel shows a transient Save confirmation here instead of
        // a second hint row.
        .overlay(alignment: .bottom) {
            if savedToast {
                HStack(spacing: 6) {
                    Image(systemName: "checkmark.circle.fill")
                    Text("Saved")
                }
                .font(themeStore.uiFont(size: 12, weight: .semibold))
                .foregroundStyle(themeStore.onAccentColor())
                .padding(.horizontal, 12)
                .padding(.vertical, 7)
                .background(themeStore.accentColor(), in: Capsule())
                .padding(.bottom, 8)
                .transition(.opacity)
            }
        }
        .animation(.easeInOut(duration: 0.2), value: savedToast)
        .background(
            TodoKeyRecognizer(
                onTogglePage: { page = (page == .tasks) ? .analytics : .tasks },
                onSave: save
            ))
        // The launcher does not focus /todo (it owns its own field), so
        // focus the search bar on entry and when returning to Tasks.
        .onAppear { focusSearchIfTasks() }
        .onChange(of: page) { _, _ in focusSearchIfTasks() }
    }

    private func focusSearchIfTasks() {
        guard page == .tasks else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
            if page == .tasks { searchFocused = true }
        }
    }

    private func save() {
        state.save()
        savedToast = true
        let token = UUID()
        savedToastToken = token
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) {
            if savedToastToken == token { savedToast = false }
        }
    }

    private var topInputBar: some View {
        HStack(spacing: 8) {
            Image(systemName: page == .tasks ? "magnifyingglass" : "chart.bar")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(themeStore.accentColor())

            if page == .tasks {
                TextField("Search tasks & dates", text: $search)
                    .textFieldStyle(.plain)
                    .focused($searchFocused)
                    .font(themeStore.uiFont(size: 13.5))
                    .foregroundStyle(themeStore.fontColor())

                if !search.isEmpty {
                    Button {
                        search = ""
                    } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(themeStore.mutedTextColor())
                    }
                    .buttonStyle(.plain)
                }
            } else {
                Text("Analytics & trends")
                    .font(themeStore.uiFont(size: 13.5))
                    .foregroundStyle(themeStore.secondaryTextColor())
                Spacer(minLength: 0)
                pageToggle
            }

            Text("/todo")
                .font(themeStore.uiFont(size: 11))
                .foregroundStyle(themeStore.fontColor())
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(themeStore.selectionFillColor(), in: Capsule())
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
        .todoCard(themeStore, bordered: false)
    }

    private var buttonBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "list.bullet")
                .font(.system(size: 12))
                .foregroundStyle(themeStore.mutedTextColor())
            Text("\(state.todayStat.done)/\(state.todayStat.total) done today")
                .font(themeStore.uiFont(size: 12))
                .foregroundStyle(themeStore.mutedTextColor())

            Spacer(minLength: 0)

            pageToggle

            addDateButton

            saveButton
        }
        .padding(.horizontal, 4)
    }

    private var addDateButton: some View {
        Button {
            state.addDateGroup()
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "calendar")
                    .font(.system(size: 11))
                Text(state.canAddDateGroup ? "Add date + \(state.groupsLeft)" : "Add date")
                    .font(themeStore.uiFont(size: 12, weight: .medium))
            }
            .foregroundStyle(
                state.canAddDateGroup
                    ? themeStore.secondaryTextColor() : themeStore.mutedTextColor()
            )
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .overlay(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .stroke(themeStore.borderColor(), lineWidth: 1)
            )
            .opacity(state.canAddDateGroup ? 1 : 0.5)
        }
        .buttonStyle(.plain)
        .disabled(!state.canAddDateGroup)
        .help(
            state.canAddDateGroup
                ? "\(state.groupsLeft) upcoming group\(state.groupsLeft == 1 ? "" : "s") left"
                : "Max \(TodoCommand.dateGroupLimit) upcoming groups")
    }

    private var saveButton: some View {
        Button(action: save) {
            HStack(spacing: 5) {
                Image(systemName: "square.and.arrow.down")
                    .font(.system(size: 11, weight: .semibold))
                Text("Save")
                    .font(themeStore.uiFont(size: 12, weight: .semibold))
                if state.dirty {
                    Circle()
                        .fill(themeStore.onAccentColor())
                        .frame(width: 5, height: 5)
                }
            }
            .foregroundStyle(state.dirty ? themeStore.onAccentColor() : themeStore.mutedTextColor())
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(
                state.dirty ? themeStore.accentColor() : themeStore.controlFillColor(),
                in: RoundedRectangle(cornerRadius: 7, style: .continuous)
            )
        }
        .buttonStyle(.plain)
    }

    private var pageToggle: some View {
        HStack(spacing: 2) {
            toggleSegment(.tasks, label: "Tasks", icon: "list.bullet")
            toggleSegment(.analytics, label: "Stats", icon: "chart.bar")
        }
        .padding(2)
        .background(
            themeStore.commandModeBackgroundColor(),
            in: RoundedRectangle(cornerRadius: 8, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(themeStore.borderColor(), lineWidth: 1)
        )
    }

    private func toggleSegment(_ target: TodoPage, label: String, icon: String) -> some View {
        let active = page == target
        return Button {
            page = target
        } label: {
            HStack(spacing: 5) {
                Image(systemName: icon).font(.system(size: 10))
                Text(label).font(themeStore.uiFont(size: 12, weight: active ? .semibold : .medium))
            }
            .foregroundStyle(active ? themeStore.fontColor() : themeStore.mutedTextColor())
            .padding(.horizontal, 10)
            .padding(.vertical, 3)
            .background(
                active ? themeStore.selectionFillColor() : Color.clear,
                in: RoundedRectangle(cornerRadius: 6, style: .continuous)
            )
        }
        .buttonStyle(.plain)
    }

}

struct TodoTasksPage: View {
    let themeStore: ThemeStore
    @Bindable var state: TodoState
    let search: String

    private var filteredGroups: [TodoGroup] {
        let q = search.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else { return state.groups }
        return state.groups.compactMap { g in
            let groupHay = "\(g.weekday) \(g.monthDay) \(g.relative)"
            if TodoCommand.fuzzyMatch(q, groupHay) { return g }
            let tasks = g.tasks.filter { TodoCommand.fuzzyMatch(q, $0.name) }
            guard !tasks.isEmpty else { return nil }
            var copy = g
            copy.tasks = tasks
            return copy
        }
    }

    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            LazyVStack(spacing: 8) {
                ForEach(filteredGroups) { group in
                    TodoDateGroupCard(themeStore: themeStore, state: state, group: group)
                }

                if filteredGroups.isEmpty {
                    Text("No tasks match \"\(search)\"")
                        .font(themeStore.uiFont(size: 12.5))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 24)
                }
            }
            .padding(.horizontal, 4)
            .padding(.vertical, 2)
        }
    }
}

struct TodoDateGroupCard: View {
    let themeStore: ThemeStore
    @Bindable var state: TodoState
    let group: TodoGroup

    private var isPast: Bool { group.kind == .past }
    private var isToday: Bool { group.kind == .today }
    private var atLimit: Bool { group.openCount >= TodoCommand.taskLimit }
    private var overdue: Bool { isPast }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            header
            VStack(spacing: 1) {
                ForEach(group.tasks) { task in
                    TodoTaskRow(
                        themeStore: themeStore,
                        task: task,
                        overdue: overdue,
                        canToggle: !isPast,
                        onToggle: { state.toggleTask(group: group.key, task: task.id) },
                        onRemove: { state.removeTask(group: group.key, task: task.id) },
                        onEdit: { name in
                            state.editTask(group: group.key, task: task.id, name: name)
                        }
                    )
                }
                if !isPast {
                    TodoAddRow(
                        themeStore: themeStore,
                        atLimit: atLimit,
                        onAdd: { name in state.addTask(group: group.key, name: name) }
                    )
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .todoCard(themeStore)
    }

    private var header: some View {
        HStack(spacing: 8) {
            TodoProgressRing(done: group.doneCount, total: group.total, themeStore: themeStore)

            HStack(alignment: .firstTextBaseline, spacing: 7) {
                Text(isToday ? "Today" : group.weekday)
                    .font(themeStore.uiFont(size: 13, weight: .semibold))
                    .foregroundStyle(isToday ? themeStore.accentColor() : themeStore.fontColor())
                Text(groupSubLabel)
                    .font(themeStore.uiFont(size: 11.5))
                    .foregroundStyle(themeStore.mutedTextColor())
            }

            Spacer(minLength: 0)

            if !isPast {
                TodoGhostButton(themeStore: themeStore, icon: "checklist", help: "Complete all") {
                    state.completeAll(group: group.key)
                }
                TodoGhostButton(themeStore: themeStore, icon: "trash", help: "Clear all") {
                    state.clearAll(group: group.key)
                }
            }
        }
    }

    private var groupSubLabel: String {
        let rel = group.relative
        if rel.isEmpty || rel == "Today" { return group.monthDay }
        return "\(group.monthDay) · \(rel)"
    }
}

struct TodoTaskRow: View {
    let themeStore: ThemeStore
    let task: TodoTask
    let overdue: Bool
    let canToggle: Bool
    let onToggle: () -> Void
    let onRemove: () -> Void
    let onEdit: (String) -> Void

    @State private var hover = false
    @State private var editing = false
    @State private var draft = ""
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 9) {
            TodoCheckbox(done: task.done, enabled: canToggle, themeStore: themeStore, onToggle: onToggle)

            if editing {
                TextField("", text: $draft)
                    .textFieldStyle(.plain)
                    .focused($focused)
                    .font(themeStore.uiFont(size: 13))
                    .foregroundStyle(themeStore.fontColor())
                    .onSubmit(commit)
                    .limitLength($draft, to: TodoCommand.taskNameMaxLength)
                    .onChange(of: focused) { _, isFocused in
                        if !isFocused { commit() }
                    }
            } else {
                Text(task.name)
                    .font(themeStore.uiFont(size: 13))
                    .foregroundStyle(nameColor)
                    .strikethrough(task.done, color: themeStore.mutedTextColor())
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
                    .onTapGesture { beginEdit() }
            }

            if overdue && !task.done && !editing {
                Text("OVERDUE")
                    .font(.system(size: 9, weight: .semibold, design: .monospaced))
                    .foregroundStyle(themeStore.dangerColor())
                    .padding(.horizontal, 6)
                    .padding(.vertical, 1)
                    .background(
                        themeStore.dangerColor().opacity(0.14),
                        in: RoundedRectangle(cornerRadius: 4, style: .continuous))
            }

            if !editing {
                Button(action: onRemove) {
                    Image(systemName: "xmark")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(themeStore.dangerColor())
                }
                .buttonStyle(.plain)
                .help("Remove task")
                .opacity(hover ? 1 : 0)
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(
            (editing || hover) ? themeStore.selectionFillColor() : Color.clear,
            in: RoundedRectangle(cornerRadius: 6, style: .continuous)
        )
        .onHover { hover = $0 }
    }

    private var nameColor: Color {
        if task.done { return themeStore.mutedTextColor() }
        return overdue ? themeStore.dangerColor() : themeStore.fontColor()
    }

    private func beginEdit() {
        draft = task.name
        editing = true
        DispatchQueue.main.async { focused = true }
    }

    private func commit() {
        let trimmed = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty && trimmed != task.name { onEdit(trimmed) }
        editing = false
    }
}

struct TodoAddRow: View {
    let themeStore: ThemeStore
    let atLimit: Bool
    let onAdd: (String) -> Void

    @State private var active = false
    @State private var draft = ""
    @FocusState private var focused: Bool

    var body: some View {
        if atLimit {
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .strokeBorder(style: StrokeStyle(lineWidth: 1.5, dash: [2, 2]))
                    .frame(width: 16, height: 16)
                    .foregroundStyle(themeStore.mutedTextColor())
                Text("\(TodoCommand.taskLimit) unfinished · complete one to add more")
                    .font(themeStore.uiFont(size: 11.5))
                    .foregroundStyle(themeStore.mutedTextColor())
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
        } else if active {
            HStack(spacing: 9) {
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .strokeBorder(themeStore.accentColor(), lineWidth: 1.5)
                    .frame(width: 16, height: 16)
                TextField("Task name, then ↵", text: $draft)
                    .textFieldStyle(.plain)
                    .focused($focused)
                    .font(themeStore.uiFont(size: 13))
                    .foregroundStyle(themeStore.fontColor())
                    .onSubmit(commit)
                    .limitLength($draft, to: TodoCommand.taskNameMaxLength)
                    .onChange(of: focused) { _, isFocused in
                        if !isFocused && draft.trimmingCharacters(in: .whitespaces).isEmpty {
                            active = false
                        }
                    }
                Text("↵ add · esc")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(themeStore.mutedTextColor())
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(
                themeStore.selectionFillColor(),
                in: RoundedRectangle(cornerRadius: 6, style: .continuous))
        } else {
            Button {
                active = true
                DispatchQueue.main.async { focused = true }
            } label: {
                HStack(spacing: 9) {
                    RoundedRectangle(cornerRadius: 5, style: .continuous)
                        .strokeBorder(style: StrokeStyle(lineWidth: 1.5, dash: [2, 2]))
                        .frame(width: 16, height: 16)
                        .overlay(Image(systemName: "plus").font(.system(size: 8, weight: .bold)))
                        .foregroundStyle(themeStore.mutedTextColor())
                    Text("Add task")
                        .font(themeStore.uiFont(size: 13))
                        .foregroundStyle(themeStore.mutedTextColor())
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
    }

    private func commit() {
        let trimmed = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty { onAdd(trimmed) }
        draft = ""
        // Keep the field active for rapid entry of several tasks.
        DispatchQueue.main.async { focused = true }
    }
}

struct TodoCheckbox: View {
    let done: Bool
    var enabled: Bool = true
    let themeStore: ThemeStore
    let onToggle: () -> Void

    var body: some View {
        Button(action: { if enabled { onToggle() } }) {
            box
                // The unchecked box is a clear fill, which SwiftUI does not
                // reliably hit-test. Back it with an opaque hit region and
                // pad it out so clicks near the box still land.
                .padding(4)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(-4)
        .disabled(!enabled)
        // Past days are read-only for completion state; the name stays
        // editable but the box cannot be toggled.
        .help(enabled ? "" : "Past days are read-only")
    }

    private var box: some View {
        RoundedRectangle(cornerRadius: 5, style: .continuous)
            .fill(done ? themeStore.accentColor() : Color.clear)
            .frame(width: 16, height: 16)
            .overlay(
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .stroke(borderColor, lineWidth: 1.5)
            )
            .overlay {
                if done {
                    Image(systemName: "checkmark")
                        .font(.system(size: 9, weight: .heavy))
                        .foregroundStyle(themeStore.onAccentColor())
                }
            }
            .opacity(enabled ? 1 : 0.55)
    }

    private var borderColor: Color {
        done ? themeStore.accentColor() : themeStore.mutedTextColor()
    }
}

struct TodoProgressRing: View {
    let done: Int
    let total: Int
    let themeStore: ThemeStore
    var size: CGFloat = 18

    private var fraction: Double { total > 0 ? Double(done) / Double(total) : 0 }

    var body: some View {
        ZStack {
            Circle()
                .stroke(themeStore.dividerColor(), lineWidth: 2.5)
            Circle()
                .trim(from: 0, to: max(0, min(1, fraction)))
                .stroke(
                    themeStore.accentColor(), style: StrokeStyle(lineWidth: 2.5, lineCap: .round)
                )
                .rotationEffect(.degrees(-90))
        }
        .frame(width: size, height: size)
    }
}

struct TodoGhostButton: View {
    let themeStore: ThemeStore
    let icon: String
    let help: String
    let action: () -> Void

    @State private var hover = false

    var body: some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: 12))
                .foregroundStyle(hover ? themeStore.fontColor() : themeStore.mutedTextColor())
                .padding(4)
                .background(
                    hover ? themeStore.controlFillColor() : Color.clear,
                    in: RoundedRectangle(cornerRadius: 6, style: .continuous)
                )
        }
        .buttonStyle(.plain)
        .help(help)
        .onHover { hover = $0 }
    }
}

// Mirrors PomoView's KeyCommandsRecognizer: a local NSEvent monitor
// installed only while the panel is on screen. Only fires on the ⌘
// chord, so it never steals plain typing from the search / add fields.

struct TodoKeyRecognizer: NSViewRepresentable {
    var onTogglePage: () -> Void
    var onSave: () -> Void

    func makeNSView(context: Context) -> TodoKeyHostView {
        let v = TodoKeyHostView()
        v.onTogglePage = onTogglePage
        v.onSave = onSave
        return v
    }

    func updateNSView(_ nsView: TodoKeyHostView, context: Context) {
        nsView.onTogglePage = onTogglePage
        nsView.onSave = onSave
    }
}

final class TodoKeyHostView: NSView {
    var onTogglePage: (() -> Void)?
    var onSave: (() -> Void)?
    nonisolated(unsafe) private var monitor: Any?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil { install() } else { remove() }
    }

    deinit {
        if let monitor { NSEvent.removeMonitor(monitor) }
    }

    private func install() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self else { return event }
            let mods = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            guard mods == .command else { return event }
            let chars = event.charactersIgnoringModifiers?.lowercased() ?? ""
            if chars == "n" {
                self.onTogglePage?()
                return nil
            }
            if chars == "s" {
                self.onSave?()
                return nil
            }
            return event
        }
    }

    private func remove() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
}

// Shared surface for /todo cards and controls: a control-fill background
// with rounded corners and an optional hairline border.
extension View {
    func todoCard(_ themeStore: ThemeStore, cornerRadius: CGFloat = 10, bordered: Bool = true) -> some View {
        background(
            themeStore.controlFillColor(),
            in: RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
        )
        .overlay {
            if bordered {
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .stroke(themeStore.borderColor(), lineWidth: 1)
            }
        }
    }

    /// Clamps a text binding to `max` characters as the user types.
    func limitLength(_ text: Binding<String>, to max: Int) -> some View {
        onChange(of: text.wrappedValue) { _, value in
            if value.count > max { text.wrappedValue = String(value.prefix(max)) }
        }
    }
}

// Thin vertical rule used between columns in the analytics strips.
struct TodoVDivider: View {
    let themeStore: ThemeStore
    var body: some View {
        Rectangle().fill(themeStore.dividerColor()).frame(width: 1).padding(.vertical, 4)
    }
}
