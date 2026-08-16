import Contacts
import Foundation

/// A parsed "call ..." line. Mirrors `look_ai::calling::CallRequest`.
nonisolated struct CallRequest: Decodable, Equatable {
    /// The words naming the person, to match against Contacts.
    let name: String
    /// A `Modality` id, or nil when the line did not say and the default
    /// applies.
    let modality: String?
}

/// One way to reach a person: a handle, and what it can be used for.
nonisolated struct ContactHandle: Equatable, Identifiable {
    /// A `Modality` id from `look_ai::calling`, chosen when the handle was read
    /// (a phone number can message or call; an email can only FaceTime).
    let modalityID: String
    let modalityLabel: String
    /// As Contacts stores it, for display. `call_url` normalises it for dialling.
    let handle: String
    /// The Contacts label ("mobile", "work"), when there is one.
    let handleLabel: String?

    var id: String { "\(modalityID)|\(handle)" }
}

/// A person Look could reach, with every way to reach them.
nonisolated struct ContactMatch: Equatable, Identifiable {
    let id: String
    let name: String
    let handles: [ContactHandle]
}

/// Contacts lookup for the `call` tier. Reads only what a call needs - a name,
/// phone numbers, and email addresses - and never leaves the machine.
nonisolated final class ContactsService: @unchecked Sendable {
    static let shared = ContactsService()

    private enum Metrics {
        /// Enough to fill a picker without turning a common first name into a
        /// wall. Beyond this the user should type more of the name.
        static let matchLimit = 8
        /// Mirrors `MeetingService`: the launcher's call row is a computed
        /// property, so an uncached lookup would hit Contacts several times per
        /// keystroke.
        static let cacheTTL: TimeInterval = 5
    }

    private let store = CNContactStore()
    private let lock = NSLock()
    private var cached: [ContactMatch] = []
    private var cachedName = ""
    private var cachedAt = Date.distantPast

    private init() {}

    var access: CalendarAccess {
        switch CNContactStore.authorizationStatus(for: .contacts) {
        case .authorized: return .authorized
        case .notDetermined: return .notDetermined
        case .denied: return .denied
        case .restricted: return .restricted
        // `.limited` (macOS 26) is partial access to a chosen subset. Treated as
        // authorized: what it hands over is what Look can act on.
        @unknown default: return .authorized
        }
    }

    func requestAccess() async {
        _ = try? await store.requestAccess(for: .contacts)
    }

    /// People whose name matches `name`, each with the handles a call can use.
    /// Empty without access, so the caller must check `access` first to tell
    /// "no such person" from "Look cannot look".
    func matches(name: String, now: Date = Date()) -> [ContactMatch] {
        guard access == .authorized, !name.trimmingCharacters(in: .whitespaces).isEmpty else {
            return []
        }
        lock.lock()
        defer { lock.unlock() }
        if name == cachedName, now.timeIntervalSince(cachedAt) < Metrics.cacheTTL {
            return cached
        }
        cachedAt = now
        cachedName = name
        cached = fetch(name: name)
        return cached
    }

    private func fetch(name: String) -> [ContactMatch] {
        let keys: [CNKeyDescriptor] = [
            CNContactFormatter.descriptorForRequiredKeys(for: .fullName),
            CNContactPhoneNumbersKey as CNKeyDescriptor,
            CNContactEmailAddressesKey as CNKeyDescriptor,
        ]
        let predicate = CNContact.predicateForContacts(matchingName: name)
        let found = (try? store.unifiedContacts(matching: predicate, keysToFetch: keys)) ?? []

        return found.prefix(Metrics.matchLimit).compactMap { contact in
            let display = CNContactFormatter.string(from: contact, style: .fullName)
            let name = display?.trimmingCharacters(in: .whitespaces) ?? ""
            let handles = Self.handles(of: contact)
            // A contact with no phone and no email cannot be called at all, so
            // it is not a match - it would be a row that does nothing.
            guard !name.isEmpty, !handles.isEmpty else { return nil }
            return ContactMatch(id: contact.identifier, name: name, handles: handles)
        }
    }

    /// Phone numbers first (they can do everything), then emails, which reach
    /// FaceTime only - `sms:` to an address is not a thing.
    private static func handles(of contact: CNContact) -> [ContactHandle] {
        var handles: [ContactHandle] = []
        for number in contact.phoneNumbers {
            let value = number.value.stringValue
            let label = number.label.map { CNLabeledValue<NSString>.localizedString(forLabel: $0) }
            for (id, title) in [
                ("message", "Message"),
                ("face_time_audio", "FaceTime audio"),
                ("face_time_video", "FaceTime video"),
            ] {
                handles.append(
                    ContactHandle(
                        modalityID: id, modalityLabel: title, handle: value, handleLabel: label))
            }
        }
        for email in contact.emailAddresses {
            let value = email.value as String
            let label = email.label.map { CNLabeledValue<NSString>.localizedString(forLabel: $0) }
            for (id, title) in [
                ("face_time_audio", "FaceTime audio"),
                ("face_time_video", "FaceTime video"),
            ] {
                handles.append(
                    ContactHandle(
                        modalityID: id, modalityLabel: title, handle: value, handleLabel: label))
            }
        }
        return handles
    }
}
