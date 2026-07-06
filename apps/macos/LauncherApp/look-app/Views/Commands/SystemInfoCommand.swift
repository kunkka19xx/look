import Foundation
import AppKit
import IOKit
import IOKit.ps
import SwiftUI

struct SystemInfoItem: Identifiable {
    let id = UUID()
    let label: String
    let value: String
    let isHeader: Bool
}

struct SystemInfoView: View {
    let themeStore: ThemeStore
    @State private var items: [SystemInfoItem] = []
    @State private var lastCPULoad: host_cpu_load_info?
    @State private var refreshTask: Task<Void, Never>?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(items) { item in
                    if item.isHeader {
                        Text(item.label)
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .bold))
                            .foregroundStyle(themeStore.fontColor())
                            .padding(.top, 4)
                    } else if item.label.isEmpty {
                        Text(item.value)
                            .font(.system(size: CGFloat(themeStore.settings.fontSize - 1), design: .monospaced))
                            .foregroundStyle(themeStore.secondaryTextColor())
                    } else {
                        HStack(alignment: .top, spacing: 4) {
                            Text(item.label)
                                .font(.system(size: CGFloat(themeStore.settings.fontSize - 1), design: .monospaced))
                                .foregroundStyle(themeStore.mutedTextColor())
                                .frame(minWidth: 70, alignment: .leading)
                            Text(item.value)
                                .font(.system(size: CGFloat(themeStore.settings.fontSize - 1), design: .monospaced))
                                .foregroundStyle(themeStore.secondaryTextColor())
                        }
                    }
                }
            }
            .padding(4)
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .onAppear {
            startRefreshing()
        }
        .onDisappear {
            refreshTask?.cancel()
            refreshTask = nil
        }
    }

    private func startRefreshing() {
        refreshTask?.cancel()
        refreshTask = Task { @MainActor in
            // first paint immediately so the panel never shows empty
            await refreshOnce()
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                if Task.isCancelled { break }
                await refreshOnce()
            }
        }
    }

    @MainActor
    private func refreshOnce() async {
        let previous = lastCPULoad
        let snapshot = await Task.detached(priority: .utility) {
            SystemInfoCommand.snapshot(previousCPULoad: previous)
        }.value
        items = snapshot.items
        lastCPULoad = snapshot.cpuLoad
    }
}

nonisolated enum SystemInfoCommand {
    struct Snapshot {
        let items: [SystemInfoItem]
        let cpuLoad: host_cpu_load_info?
    }

    // Pure function - safe to call off the main actor. The caller owns the
    // previous CPU sample so we don't need shared mutable state.
    static func snapshot(previousCPULoad: host_cpu_load_info?) -> Snapshot {
        var items: [SystemInfoItem] = []
        items.append(SystemInfoItem(label: "", value: "System Info", isHeader: true))
        items.append(contentsOf: getSystemOverviewItems())
        items.append(contentsOf: getMemoryItems())

        let currentLoad = sampleCPULoad()
        items.append(contentsOf: getCPUItems(currentLoad: currentLoad, previousLoad: previousCPULoad))

        if let batteryItems = getBatteryItems() {
            items.append(contentsOf: batteryItems)
        }
        items.append(contentsOf: getUptimeItems())
        items.append(contentsOf: getDiskItems())
        return Snapshot(items: items, cpuLoad: currentLoad)
    }

    static func getSystemInfo() -> String {
        let items = snapshot(previousCPULoad: nil).items
        var lines: [String] = []
        for item in items {
            if item.isHeader {
                lines.append(item.value)
            } else if item.label.isEmpty {
                lines.append(item.value)
            } else {
                lines.append("\(item.label): \(item.value)")
            }
        }
        return lines.joined(separator: "\n")
    }

    static func getSystemInfoItems() -> [SystemInfoItem] {
        snapshot(previousCPULoad: nil).items
    }

    private static func getSystemOverviewItems() -> [SystemInfoItem] {
        let model = getModelIdentifier()
        let osVersion = ProcessInfo.processInfo.operatingSystemVersion
        let osName = getMacOSName(osVersion: osVersion)
        return [
            SystemInfoItem(label: "Model", value: model, isHeader: false),
            SystemInfoItem(label: "macOS", value: "\(osName) \(osVersion.majorVersion).\(osVersion.minorVersion).\(osVersion.patchVersion)", isHeader: false)
        ]
    }

    private static func getModelIdentifier() -> String {
        var size = 0
        sysctlbyname("hw.model", nil, &size, nil, 0)
        var model = [CChar](repeating: 0, count: size)
        sysctlbyname("hw.model", &model, &size, nil, 0)
        return cStringArrayToString(model)
    }

    private static func getMacOSName(osVersion: OperatingSystemVersion) -> String {
        let major = osVersion.majorVersion
        if major >= 15 {
            return "Sequoia"
        } else if major >= 14 {
            return "Sonoma"
        } else if major >= 13 {
            return "Ventura"
        } else if major >= 12 {
            return "Monterey"
        } else if major >= 11 {
            return "Big Sur"
        } else {
            return "macOS"
        }
    }

    private static func getMemoryItems() -> [SystemInfoItem] {
        let physicalMem = ProcessInfo.processInfo.physicalMemory
        let physicalGB = Double(physicalMem) / (1024 * 1024 * 1024)

        var vmStats = vm_statistics64()
        var count = mach_msg_type_number_t(MemoryLayout<vm_statistics64>.size / MemoryLayout<integer_t>.size)
        let hostPort = mach_host_self()

        let result = withUnsafeMutablePointer(to: &vmStats) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                host_statistics64(hostPort, HOST_VM_INFO64, $0, &count)
            }
        }

        var usedMB: Double = 0
        var cachedMB: Double = 0

        if result == KERN_SUCCESS {
            // vm_kernel_page_size is a global var; Swift 6 flags it as
            // not concurrency-safe. Use the host_page_size() function
            // (process-local, sendable-safe) instead.
            var pageSize: vm_size_t = 0
            host_page_size(mach_host_self(), &pageSize)
            let activePages = Double(vmStats.active_count)
            let wirePages = Double(vmStats.wire_count)
            let compressedPages = Double(vmStats.compressor_page_count)
            let inactivePages = Double(vmStats.inactive_count)

            usedMB = (activePages + wirePages) * Double(pageSize) / (1024 * 1024)
            cachedMB = (inactivePages + compressedPages) * Double(pageSize) / (1024 * 1024)
        }

        return [
            SystemInfoItem(label: "", value: "Memory", isHeader: true),
            SystemInfoItem(label: "Total", value: String(format: "%.1f GB", physicalGB), isHeader: false),
            SystemInfoItem(label: "Used", value: String(format: "%.0f MB", usedMB), isHeader: false),
            SystemInfoItem(label: "Cached", value: String(format: "%.0f MB", cachedMB), isHeader: false)
        ]
    }

    private static func getCPUItems(currentLoad: host_cpu_load_info?, previousLoad: host_cpu_load_info?) -> [SystemInfoItem] {
        var cpuBrand = "Unknown"
        let cpuCount = ProcessInfo.processInfo.processorCount
        let cpuCores = ProcessInfo.processInfo.activeProcessorCount

        var size = 0
        sysctlbyname("machdep.cpu.brand_string", nil, &size, nil, 0)
        if size > 0 {
            var brand = [CChar](repeating: 0, count: size)
            sysctlbyname("machdep.cpu.brand_string", &brand, &size, nil, 0)
            cpuBrand = cStringArrayToString(brand)
        }

        if cpuBrand == "Unknown" {
            size = 0
            sysctlbyname("hw.machine", nil, &size, nil, 0)
            var machine = [CChar](repeating: 0, count: size)
            sysctlbyname("hw.machine", &machine, &size, nil, 0)
            cpuBrand = cStringArrayToString(machine)
        }

        let usage = formatCPUUsage(current: currentLoad, previous: previousLoad)

        return [
            SystemInfoItem(label: "", value: "CPU", isHeader: true),
            SystemInfoItem(label: "Chip", value: cpuBrand, isHeader: false),
            SystemInfoItem(label: "Cores", value: "\(cpuCores) (\(cpuCount) logical)", isHeader: false),
            SystemInfoItem(label: "Usage", value: "\(usage)%", isHeader: false)
        ]
    }

    static func sampleCPULoad() -> host_cpu_load_info? {
        var load = host_cpu_load_info()
        var count = mach_msg_type_number_t(MemoryLayout<host_cpu_load_info>.size / MemoryLayout<integer_t>.size)
        let result = withUnsafeMutablePointer(to: &load) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                host_statistics(mach_host_self(), HOST_CPU_LOAD_INFO, $0, &count)
            }
        }
        return result == KERN_SUCCESS ? load : nil
    }

    // host_cpu_load_info gives cumulative ticks since boot; usage % needs the
    // delta between two samples. The first refresh after the panel opens has
    // no prior sample and shows "-".
    private static func formatCPUUsage(current: host_cpu_load_info?, previous: host_cpu_load_info?) -> String {
        guard let current else { return "N/A" }
        guard let previous else { return "-" }

        let userDelta = Double(current.cpu_ticks.0 &- previous.cpu_ticks.0)
        let systemDelta = Double(current.cpu_ticks.1 &- previous.cpu_ticks.1)
        let idleDelta = Double(current.cpu_ticks.2 &- previous.cpu_ticks.2)
        let niceDelta = Double(current.cpu_ticks.3 &- previous.cpu_ticks.3)

        let total = userDelta + systemDelta + idleDelta + niceDelta
        guard total > 0 else { return "0.0" }
        let usage = (userDelta + systemDelta + niceDelta) / total * 100
        return String(format: "%.1f", usage)
    }

    private static func getBatteryItems() -> [SystemInfoItem]? {
        guard let snapshotRef = IOPSCopyPowerSourcesInfo()?.takeRetainedValue() else {
            return nil
        }
        guard let sources = IOPSCopyPowerSourcesList(snapshotRef)?.takeRetainedValue() as? [CFTypeRef] else {
            return nil
        }

        for source in sources {
            guard let desc = IOPSGetPowerSourceDescription(snapshotRef, source)?.takeUnretainedValue() as? [String: Any] else {
                continue
            }
            guard let type = desc[kIOPSTypeKey as String] as? String,
                  type == kIOPSInternalBatteryType as String
            else { continue }

            var items: [SystemInfoItem] = [SystemInfoItem(label: "", value: "Battery", isHeader: true)]

            if let current = desc[kIOPSCurrentCapacityKey as String] as? Int,
               let maxCap = desc[kIOPSMaxCapacityKey as String] as? Int, maxCap > 0 {
                let pct = Int(Double(current) / Double(maxCap) * 100)
                items.append(SystemInfoItem(label: "Charge", value: "\(pct)%", isHeader: false))
            }

            let status: String
            let isCharging = desc[kIOPSIsChargingKey as String] as? Bool ?? false
            let isCharged = desc[kIOPSIsChargedKey as String] as? Bool ?? false
            let powerState = desc[kIOPSPowerSourceStateKey as String] as? String ?? ""
            if isCharged {
                status = "Charged"
            } else if isCharging {
                status = "Charging"
            } else if powerState == kIOPSACPowerValue as String {
                status = "AC"
            } else {
                status = "Discharging"
            }
            items.append(SystemInfoItem(label: "Status", value: status, isHeader: false))

            return items
        }

        return nil
    }

    private static func getUptimeItems() -> [SystemInfoItem] {
        let uptime = ProcessInfo.processInfo.systemUptime
        let days = Int(uptime) / 86400
        let hours = (Int(uptime) % 86400) / 3600
        let minutes = (Int(uptime) % 3600) / 60

        var uptimeStr = ""
        if days > 0 {
            uptimeStr += "\(days)d "
        }
        uptimeStr += "\(hours)h \(minutes)m"

        return [
            SystemInfoItem(label: "", value: "Uptime", isHeader: true),
            SystemInfoItem(label: "Time", value: uptimeStr, isHeader: false)
        ]
    }

    private static func getDiskItems() -> [SystemInfoItem] {
        var items: [SystemInfoItem] = [SystemInfoItem(label: "", value: "Disk", isHeader: true)]

        let fileManager = FileManager.default
        guard let volumes = fileManager.mountedVolumeURLs(
            includingResourceValuesForKeys: [.volumeNameKey, .volumeTotalCapacityKey, .volumeAvailableCapacityForImportantUsageKey],
            options: [.skipHiddenVolumes]
        ) else {
            return items
        }

        for volumeURL in volumes {
            guard let resourceValues = try? volumeURL.resourceValues(forKeys: [
                .volumeNameKey, .volumeTotalCapacityKey, .volumeAvailableCapacityForImportantUsageKey
            ]) else {
                continue
            }

            let name = resourceValues.volumeName ?? volumeURL.lastPathComponent
            let totalGB = Double(resourceValues.volumeTotalCapacity ?? 0) / (1024 * 1024 * 1024)
            let freeGB = Double(resourceValues.volumeAvailableCapacityForImportantUsage ?? 0) / (1024 * 1024 * 1024)
            let usedPercent = totalGB > 0 ? ((totalGB - freeGB) / totalGB) * 100 : 0

            items.append(SystemInfoItem(
                label: name,
                value: String(format: "%.0fGB / %.0fGB (%.0f%%)", freeGB, totalGB, usedPercent),
                isHeader: false
            ))
        }

        return items
    }

    private static func cStringArrayToString(_ buffer: [CChar]) -> String {
        let bytes = buffer.prefix { $0 != 0 }.map { UInt8(bitPattern: $0) }
        return String(decoding: bytes, as: UTF8.self)
    }
}
