import Foundation
import AppKit
import IOKit
import SwiftUI

struct SystemInfoItem: Identifiable {
    let id = UUID()
    let label: String
    let value: String
    let isHeader: Bool
}

struct SystemInfoView: View {
    let items: [SystemInfoItem]
    let themeStore: ThemeStore

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
    }

    static func buildItems() -> [SystemInfoItem] {
        var items: [SystemInfoItem] = []

        items.append(SystemInfoItem(label: "", value: "System Info", isHeader: true))
        items.append(contentsOf: getSystemOverviewItems())
        items.append(contentsOf: getMemoryItems())
        items.append(contentsOf: getCPUItems())
        if let batteryItems = getBatteryItems() {
            items.append(contentsOf: batteryItems)
        }
        items.append(contentsOf: getUptimeItems())
        items.append(contentsOf: getDiskItems())

        return items
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
        return String(cString: model)
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
            let pageSize = vm_kernel_page_size
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

    private static func getCPUItems() -> [SystemInfoItem] {
        var cpuBrand = "Unknown"
        let cpuCount = ProcessInfo.processInfo.processorCount
        let cpuCores = ProcessInfo.processInfo.activeProcessorCount

        var size = 0
        sysctlbyname("machdep.cpu.brand_string", nil, &size, nil, 0)
        if size > 0 {
            var brand = [CChar](repeating: 0, count: size)
            sysctlbyname("machdep.cpu.brand_string", &brand, &size, nil, 0)
            cpuBrand = String(cString: brand)
        }

        if cpuBrand == "Unknown" {
            size = 0
            sysctlbyname("hw.machine", nil, &size, nil, 0)
            var machine = [CChar](repeating: 0, count: size)
            sysctlbyname("hw.machine", &machine, &size, nil, 0)
            cpuBrand = String(cString: machine)
        }

        let usage = getCPUUsage()

        return [
            SystemInfoItem(label: "", value: "CPU", isHeader: true),
            SystemInfoItem(label: "Chip", value: cpuBrand, isHeader: false),
            SystemInfoItem(label: "Cores", value: "\(cpuCores) (\(cpuCount) logical)", isHeader: false),
            SystemInfoItem(label: "Usage", value: "\(usage)%", isHeader: false)
        ]
    }

    private static func getCPUUsage() -> String {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/top")
        task.arguments = ["-l", "1", "-n", "0"]

        let pipe = Pipe()
        task.standardOutput = pipe
        task.standardError = FileHandle.nullDevice

        do {
            try task.run()
            task.waitUntilExit()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            guard let output = String(data: data, encoding: .utf8) else {
                return "N/A"
            }

            guard let cpuLine = output
                .components(separatedBy: "\n")
                .first(where: { $0.lowercased().contains("cpu usage") })
            else {
                return "N/A"
            }

            let pattern = "([0-9]+(?:\\.[0-9]+)?)% user,\\s*([0-9]+(?:\\.[0-9]+)?)% sys"
            guard let regex = try? NSRegularExpression(pattern: pattern) else {
                return "N/A"
            }
            let range = NSRange(cpuLine.startIndex..<cpuLine.endIndex, in: cpuLine)
            guard let match = regex.firstMatch(in: cpuLine, range: range),
                  let userRange = Range(match.range(at: 1), in: cpuLine),
                  let sysRange = Range(match.range(at: 2), in: cpuLine),
                  let user = Double(cpuLine[userRange]),
                  let sys = Double(cpuLine[sysRange]) else {
                return "N/A"
            }

            return String(format: "%.1f", user + sys)
        } catch {
            return "N/A"
        }
    }

    private static func getBatteryItems() -> [SystemInfoItem]? {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/pmset")
        task.arguments = ["-g", "batt"]

        let pipe = Pipe()
        task.standardOutput = pipe
        task.standardError = FileHandle.nullDevice

        do {
            try task.run()
            task.waitUntilExit()

            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            if let output = String(data: data, encoding: .utf8) {
                let lines = output.components(separatedBy: "\n")
                var items: [SystemInfoItem] = [SystemInfoItem(label: "", value: "Battery", isHeader: true)]

                for line in lines {
                    let trimmed = line.trimmingCharacters(in: .whitespaces)
                    if trimmed.contains("%") && !trimmed.hasPrefix("Now") {
                        let parts = trimmed.components(separatedBy: " ").filter { !$0.isEmpty }
                        for part in parts {
                            if part.contains("%") {
                                items.append(SystemInfoItem(label: "Charge", value: part, isHeader: false))
                            } else if part == "charging" || part == "discharging" || part == "charged" {
                                items.append(SystemInfoItem(label: "Status", value: part.capitalized, isHeader: false))
                            }
                        }
                    }
                }

                return items.isEmpty ? nil : items
            }
        } catch {
            return nil
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
}

struct SystemInfoCommand {
    static func getSystemInfo() -> String {
        let items = SystemInfoView.buildItems()
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
        return SystemInfoView.buildItems()
    }
}
