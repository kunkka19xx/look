import AppKit
import UniformTypeIdentifiers

/// How a planner-proposed action presents across the launcher: row icon, type
/// badge, and verb, keyed by tool id. The main-bar action row, its preview
/// panel, and the hint lines all read from here, so supporting a new tool is
/// one entry - never another hand-styled row.
enum AIActionAppearance {
    struct Look {
        let symbol: String
        let typeName: String
        let verb: String
    }

    static func look(forToolID toolID: String) -> Look {
        switch toolID {
        case "calendar.add_event":
            Look(symbol: "calendar.badge.plus", typeName: "Event", verb: "Add to Calendar")
        case "calendar.block_time":
            Look(symbol: "timer", typeName: "Focus", verb: "Block time")
        case "calendar.move_event":
            Look(symbol: "calendar.badge.clock", typeName: "Event", verb: "Reschedule")
        case "calendar.cancel_event":
            Look(symbol: "calendar.badge.minus", typeName: "Event", verb: "Remove from Calendar")
        case "reminder.add":
            Look(symbol: "checklist", typeName: "Reminder", verb: "Add Reminder")
        case "reminder.complete":
            Look(symbol: "checkmark.circle", typeName: "Reminder", verb: "Mark Done")
        case "reminder.remove":
            Look(symbol: "minus.circle", typeName: "Reminder", verb: "Remove Reminder")
        case "reminder.snooze":
            Look(symbol: "clock.badge", typeName: "Reminder", verb: "Snooze")
        default:
            Look(symbol: "sparkles", typeName: "Action", verb: "Run")
        }
    }

    static func icon(forToolID toolID: String) -> NSImage {
        NSImage(systemSymbolName: look(forToolID: toolID).symbol, accessibilityDescription: nil)
            ?? NSImage(systemSymbolName: "sparkles", accessibilityDescription: nil)
            ?? NSWorkspace.shared.icon(for: .plainText)
    }
}
