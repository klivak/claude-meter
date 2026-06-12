import AppKit
import Foundation
import UniformTypeIdentifiers

struct Metric {
    let key: String
    let name: String
    let percent: Int
    let resetsAt: String?
}

struct Status {
    let state: String
    let title: String
    let detail: String
    let percent: Int?
    let metrics: [Metric]
    let lastApiUpdate: String?
    let error: String?

    static let loading = Status(
        state: "refreshing",
        title: "...",
        detail: "Starting ClaudeMeter",
        percent: nil,
        metrics: [],
        lastApiUpdate: nil,
        error: nil
    )
}

@main
enum ClaudeMeterMain {
    // Strong reference: NSApplication.delegate is weak, so the delegate must be
    // retained for the lifetime of the run loop. main() never returns, so this
    // stack-held reference keeps it alive.
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        app.run()
        _ = delegate
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private var menu = NSMenu()
    private var agent: Process?
    private var timer: Timer?
    private var currentStatus = Status.loading

    private let appSupport: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        return base.appendingPathComponent("ClaudeMeter", isDirectory: true)
    }()

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        try? FileManager.default.createDirectory(at: appSupport, withIntermediateDirectories: true)
        startAgent()
        reloadStatus()
        timer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            self?.reloadStatus()
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        agent?.terminate()
    }

    private func startAgent() {
        guard agent?.isRunning != true else { return }
        let process = Process()
        process.executableURL = agentURL()
        process.arguments = ["--agent"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            agent = process
        } catch {
            writeLog("Failed to start agent: \(error)")
        }
    }

    private func refreshNow() {
        currentStatus = Status(state: "refreshing", title: "...", detail: "Refreshing...", percent: nil, metrics: [], lastApiUpdate: nil, error: nil)
        renderMenu()

        DispatchQueue.global(qos: .utility).async {
            let process = Process()
            process.executableURL = self.agentURL()
            process.arguments = ["--once"]
            process.standardOutput = FileHandle.nullDevice
            process.standardError = FileHandle.nullDevice
            do {
                try process.run()
                process.waitUntilExit()
            } catch {
                self.writeLog("Manual refresh failed: \(error)")
            }
            DispatchQueue.main.async {
                self.reloadStatus()
            }
        }
    }

    private func reloadStatus() {
        let path = appSupport.appendingPathComponent("status.json")
        guard let data = try? Data(contentsOf: path),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            currentStatus = Status.loading
            renderMenu()
            return
        }

        var metrics: [Metric] = []
        if let rawMetrics = object["metrics"] as? [[String: Any]] {
            for entry in rawMetrics {
                guard let name = entry["name"] as? String,
                      let percent = entry["percent"] as? Int else { continue }
                let key = entry["key"] as? String ?? ""
                let resetsAt = entry["resets_at"] as? String
                metrics.append(Metric(key: key, name: name, percent: percent, resetsAt: resetsAt))
            }
        }

        currentStatus = Status(
            state: object["state"] as? String ?? "unknown",
            title: object["title"] as? String ?? "...",
            detail: object["detail"] as? String ?? "No data yet",
            percent: object["percent"] as? Int,
            metrics: metrics,
            lastApiUpdate: object["last_api_update"] as? String,
            error: object["error"] as? String
        )
        renderMenu()
    }

    private func renderMenu() {
        if let icon = statusIcon() {
            statusItem.button?.image = icon
            statusItem.button?.imagePosition = .imageLeading
        } else {
            statusItem.button?.image = nil
        }
        statusItem.button?.title = menuBarTitle()
        statusItem.button?.toolTip = currentStatus.detail

        menu = NSMenu()
        menu.addItem(disabled("ClaudeMeter — \(currentStatus.detail)"))
        if !currentStatus.metrics.isEmpty {
            menu.addItem(NSMenuItem.separator())
            for metric in currentStatus.metrics {
                if let left = timeLeftLong(metric.resetsAt) {
                    menu.addItem(disabled("\(metric.name): \(metric.percent)%  ·  resets in \(left)"))
                } else {
                    menu.addItem(disabled("\(metric.name): \(metric.percent)%"))
                }
                if metric.key == "seven_day", let pace = weeklyPaceLine() {
                    menu.addItem(disabled("   \(pace)"))
                }
            }
        }
        menu.addItem(NSMenuItem.separator())
        menu.addItem(disabled("Freshness: \(freshnessText())"))
        if let err = currentStatus.error, !err.isEmpty {
            menu.addItem(disabled("\u{26A0}\u{FE0E} \(err)"))
        }
        menu.addItem(NSMenuItem.separator())
        menu.addItem(item("Refresh Now", #selector(refreshAction)))
        menu.addItem(item("Open Claude Usage", #selector(openClaude)))
        menu.addItem(item("Check for Updates", #selector(checkForUpdates)))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(item("Open Config", #selector(openConfig)))
        menu.addItem(item("Export Config...", #selector(exportConfig)))
        menu.addItem(item("Import Config...", #selector(importConfig)))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(item(autostartInstalled() ? "Disable Autostart" : "Enable Autostart", #selector(toggleAutostart)))
        menu.addItem(item("Open Logs", #selector(openLogs)))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(item("Quit", #selector(quit)))

        statusItem.menu = menu
    }

    /// Menu bar text: 5-hour session usage with time left, then the weekly
    /// percent (no label), e.g. "30% · 1h03m | 24%". The leading severity dot
    /// is set as the button image.
    private func menuBarTitle() -> String {
        guard currentStatus.percent != nil else {
            return currentStatus.state == "error" ? "!" : "…"
        }
        guard let session = sessionMetric() else {
            return "\(currentStatus.percent ?? 0)%"
        }
        var text = "\(session.percent)%"
        if let left = timeLeftShort(session.resetsAt) {
            text += " · \(left)"
        }
        if let weekly = weeklyMetric() {
            text += " | \(weekly.percent)%"
        }
        return text
    }

    /// The 5-hour session metric drives the menu bar; fall back to the first.
    private func sessionMetric() -> Metric? {
        currentStatus.metrics.first(where: { $0.key == "five_hour" })
            ?? currentStatus.metrics.first
    }

    /// The weekly (all-models) metric, shown as the trailing percent.
    private func weeklyMetric() -> Metric? {
        currentStatus.metrics.first(where: { $0.key == "seven_day" })
    }

    /// Linear projection of where the weekly limit lands at reset, assuming
    /// usage continues at the average rate since the week began. Over 100%
    /// means you are on pace to hit the wall before the window resets.
    private func weeklyPaceLine() -> String? {
        guard let weekly = weeklyMetric(),
              let iso = weekly.resetsAt,
              let reset = parseISO(iso) else { return nil }
        let weekLen = 7.0 * 24 * 3600
        let secsLeft = reset.timeIntervalSinceNow
        guard secsLeft > 0 else { return nil }
        let frac = (weekLen - secsLeft) / weekLen
        guard frac > 0.05 else { return "Pace: too early to project" }
        let projected = Int((Double(weekly.percent) / frac).rounded())
        let dot = projected > 110 ? "🔴" : (projected >= 90 ? "🟡" : "🟢")
        return "Pace: on track for ~\(projected)% by reset  \(dot)"
    }

    /// Severity dot colored by the worst (max) limit, so the icon warns even
    /// when the session number shown is low but another limit is near full.
    private func statusIcon() -> NSImage? {
        let name: String
        switch currentStatus.percent {
        case .some(let p) where p >= 90: name = "dot_red"
        case .some(let p) where p >= 60: name = "dot_yellow"
        case .some: name = "dot_green"
        case .none: name = "dot_gray"
        }
        guard let url = Bundle.main.url(forResource: name, withExtension: "png"),
              let img = NSImage(contentsOf: url) else { return nil }
        img.size = NSSize(width: 14, height: 14)
        return img
    }

    private func parseISO(_ iso: String) -> Date? {
        let frac = ISO8601DateFormatter()
        frac.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let d = frac.date(from: iso) { return d }
        let plain = ISO8601DateFormatter()
        plain.formatOptions = [.withInternetDateTime]
        return plain.date(from: iso)
    }

    /// Compact countdown for the menu bar: "45m", "1h03m", "5d10h".
    private func timeLeftShort(_ iso: String?) -> String? {
        guard let iso = iso, let date = parseISO(iso) else { return nil }
        let secs = Int(date.timeIntervalSinceNow)
        if secs <= 0 { return nil }
        let h = secs / 3600, m = (secs % 3600) / 60
        if h >= 24 { return "\(h / 24)d\(h % 24)h" }
        if h >= 1 { return String(format: "%dh%02dm", h, m) }
        return "\(m)m"
    }

    /// Readable countdown for the dropdown: "45m", "1h 3m", "5d 10h".
    private func timeLeftLong(_ iso: String?) -> String? {
        guard let iso = iso, let date = parseISO(iso) else { return nil }
        let secs = Int(date.timeIntervalSinceNow)
        if secs <= 0 { return nil }
        let h = secs / 3600, m = (secs % 3600) / 60
        if h >= 24 { return "\(h / 24)d \(h % 24)h" }
        if h >= 1 { return "\(h)h \(m)m" }
        return "\(m)m"
    }

    private func freshnessText() -> String {
        guard let iso = currentStatus.lastApiUpdate,
              let date = parseISO(iso) else {
            return currentStatus.state == "error" ? "API error" : "Cached / no API data yet"
        }
        let age = max(0, Int(Date().timeIntervalSince(date)))
        if age < 20 { return "Live" }
        if age < 60 { return "\(age)s old" }
        return "\(age / 60)m old"
    }

    private func disabled(_ title: String) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        item.isEnabled = false
        return item
    }

    private func item(_ title: String, _ action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        return item
    }

    @objc private func refreshAction() {
        refreshNow()
    }

    @objc private func openClaude() {
        NSWorkspace.shared.open(URL(string: "https://claude.ai/settings/usage")!)
    }

    @objc private func checkForUpdates() {
        let url = URL(string: "https://api.github.com/repos/klivak/claude-meter/releases/latest")!
        URLSession.shared.dataTask(with: url) { data, _, _ in
            guard let data,
                  let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let tag = object["tag_name"] as? String,
                  let html = object["html_url"] as? String else {
                self.notify("ClaudeMeter", "Could not check for updates.")
                return
            }
            if tag == "v4.0.1" {
                self.notify("ClaudeMeter", "You are running the latest version.")
            } else {
                self.notify("ClaudeMeter Update", "\(tag) is available.")
                if let url = URL(string: html) {
                    NSWorkspace.shared.open(url)
                }
            }
        }.resume()
    }

    @objc private func openConfig() {
        openFile(appSupport.appendingPathComponent("config.json"))
    }

    @objc private func exportConfig() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "claudemeter-config.json"
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            let source = self.appSupport.appendingPathComponent("config.json")
            try? FileManager.default.removeItem(at: url)
            do {
                try FileManager.default.copyItem(at: source, to: url)
            } catch {
                self.notify("ClaudeMeter", "Config export failed.")
            }
        }
    }

    @objc private func importConfig() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.canChooseDirectories = false
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            let dest = self.appSupport.appendingPathComponent("config.json")
            do {
                try? FileManager.default.removeItem(at: dest)
                try FileManager.default.copyItem(at: url, to: dest)
                self.notify("ClaudeMeter", "Config imported. Restarting agent.")
                self.agent?.terminate()
                self.agent = nil
                self.startAgent()
            } catch {
                self.notify("ClaudeMeter", "Config import failed.")
            }
        }
    }

    @objc private func toggleAutostart() {
        if autostartInstalled() {
            removeAutostart()
        } else {
            installAutostart()
        }
        renderMenu()
    }

    @objc private func openLogs() {
        openFile(appSupport.appendingPathComponent("claudemeter.log"))
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }

    private func agentURL() -> URL {
        Bundle.main.resourceURL!.appendingPathComponent("claudemeter-agent")
    }

    private func openFile(_ url: URL) {
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: Data())
        }
        NSWorkspace.shared.open(url)
    }

    private func launchAgentPath() -> URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents/com.klivak.claudemeter.plist")
    }

    private func autostartInstalled() -> Bool {
        FileManager.default.fileExists(atPath: launchAgentPath().path)
    }

    private func installAutostart() {
        let appPath = Bundle.main.bundleURL.path
        let plist = """
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict>
          <key>Label</key>
          <string>com.klivak.claudemeter</string>
          <key>ProgramArguments</key>
          <array>
            <string>/usr/bin/open</string>
            <string>\(appPath)</string>
          </array>
          <key>RunAtLoad</key>
          <true/>
        </dict>
        </plist>
        """
        let path = launchAgentPath()
        try? FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? plist.write(to: path, atomically: true, encoding: .utf8)
    }

    private func removeAutostart() {
        try? FileManager.default.removeItem(at: launchAgentPath())
    }

    private func notify(_ title: String, _ message: String) {
        let script = "display notification \"\(escapeAppleScript(message))\" with title \"\(escapeAppleScript(title))\""
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-e", script]
        try? process.run()
    }

    private func writeLog(_ message: String) {
        let path = appSupport.appendingPathComponent("claudemeter.log")
        let line = "\(Date()) \(message)\n"
        if let data = line.data(using: .utf8) {
            if FileManager.default.fileExists(atPath: path.path),
               let handle = try? FileHandle(forWritingTo: path) {
                _ = try? handle.seekToEnd()
                try? handle.write(contentsOf: data)
                try? handle.close()
            } else {
                try? data.write(to: path)
            }
        }
    }

    private func escapeAppleScript(_ value: String) -> String {
        value.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
    }
}
