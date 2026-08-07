import Darwin
import Foundation

/// The address this machine holds on the local network, shown beside a speed
/// reading so the number has somewhere to belong. Read from `getifaddrs`, so it
/// costs nothing and prompts for no permission.
nonisolated enum LocalIPv4 {
    /// Interfaces are ranked by this prefix order: Wi-Fi and Ethernet come up as
    /// `en`, while `utun` (VPN) and `bridge` are only used if nothing else is.
    private static let preferredPrefix = "en"

    /// The primary IPv4 address, or nil when only loopback is up.
    static func current() -> String? {
        var head: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&head) == 0, let first = head else { return nil }
        defer { freeifaddrs(head) }

        var fallback: String?

        for interface in sequence(first: first, next: { $0.pointee.ifa_next }) {
            let flags = Int32(interface.pointee.ifa_flags)
            guard flags & IFF_UP != 0, flags & IFF_RUNNING != 0, flags & IFF_LOOPBACK == 0 else {
                continue
            }
            guard let address = interface.pointee.ifa_addr,
                address.pointee.sa_family == UInt8(AF_INET),
                let readable = numericHost(of: address)
            else {
                continue
            }

            if String(cString: interface.pointee.ifa_name).hasPrefix(preferredPrefix) {
                return readable
            }
            if fallback == nil {
                fallback = readable
            }
        }

        return fallback
    }

    private static func numericHost(of address: UnsafeMutablePointer<sockaddr>) -> String? {
        var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
        let resolved = getnameinfo(
            address,
            socklen_t(address.pointee.sa_len),
            &host,
            socklen_t(host.count),
            nil,
            0,
            NI_NUMERICHOST
        )
        guard resolved == 0 else { return nil }
        return String(cString: host)
    }
}
