import AppKit
import Combine
import SwiftUI

struct Row: Decodable {
    let section: String
    let name: String
    let state: String
    let detail: String
    var key: String { section + "\u{1F}" + name }

    var color: Color {
        switch state {
        case "present", "configured", "clean", "recent", "matches lockfile": return .green
        case "missing", "wrong target", "stale": return .red
        case "optional; not found", "manual check": return .secondary
        default: return .orange
        }
    }
}

final class Selection: ObservableObject {
    @Published var key: String?
}

struct Dashboard: View {
    @ObservedObject var data: DashboardData
    var rows: [Row] { data.rows }

    @StateObject private var selection = Selection()

    var sections: [String] { Array(Set(rows.map(\.section))).sorted() }

    struct Block: Identifiable {
        let id: Int
        let title: String
        let indices: [Int]
    }

    func column(_ blocks: [Block]) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(blocks) { block in
                VStack(alignment: .leading, spacing: 2) {
                    Text(block.title).font(.caption.bold()).foregroundStyle(.secondary)
                        .padding(.horizontal, 6).padding(.bottom, 3)
                    ForEach(block.indices, id: \.self) { index in
                        let row = rows[index]
                        Button { selection.key = row.key } label: {
                            HStack(spacing: 7) {
                                Circle().fill(row.color).frame(width: 7, height: 7)
                                    .accessibilityHidden(true)
                                Text(row.name).lineLimit(1)
                                Spacer(minLength: 0)
                            }.font(.callout).padding(.horizontal, 6).frame(height: 19)
                                .background(selection.key == row.key ? Color.accentColor.opacity(0.18) : .clear,
                                            in: RoundedRectangle(cornerRadius: 4))
                                .contentShape(Rectangle())
                        }.buttonStyle(.plain).help(row.state)
                            .accessibilityLabel("\(row.name): \(row.state)")
                    }
                }
            }
        }.frame(maxWidth: .infinity, alignment: .topLeading)
    }

    func columns(count: Int) -> [[Block]] {
        let limit = max(12, (rows.count + count - 1) / count)
        var blocks: [Block] = []
        for section in sections {
            let indices = rows.indices.filter { rows[$0].section == section }
            for start in stride(from: 0, to: indices.count, by: limit) {
                blocks.append(Block(id: blocks.count,
                                    title: start == 0 ? section : "\(section) (cont.)",
                                    indices: Array(indices[start..<min(start + limit, indices.count)])))
            }
        }
        var result = Array(repeating: [Block](), count: count)
        var sizes = Array(repeating: 0, count: count)
        for block in blocks.sorted(by: { a, b in
            a.indices.count == b.indices.count ? a.id < b.id : a.indices.count > b.indices.count
        }) {
            let index = sizes.indices.min { sizes[$0] < sizes[$1] } ?? 0
            result[index].append(block)
            sizes[index] += block.indices.count + 2
        }
        return result
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Mac Status").font(.title2.bold())
                Spacer()
                Text(data.message ?? "\(rows.count) checks · refreshes every 5s").foregroundStyle(.secondary)
            }
            GeometryReader { proxy in
                let count = max(2, min(4, Int(proxy.size.width / 210)))
                ScrollView {
                    HStack(alignment: .top, spacing: 14) {
                        ForEach(Array(columns(count: count).enumerated()), id: \.offset) { _, sections in
                            column(sections)
                        }
                    }.padding(.trailing, 8)
                }
            }
            Divider()
            VStack(alignment: .leading, spacing: 4) {
                if let row = rows.first(where: { $0.key == selection.key }) {
                    Text("\(row.name) · \(row.state)").font(.callout.bold())
                    ScrollView { Text(row.detail).font(.callout).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading) }
                } else {
                    Text("Select a check to see its status and details.").foregroundStyle(.secondary)
                }
            }.frame(height: selection.key == nil ? 20 : 58, alignment: .topLeading)
        }.padding(16).frame(minWidth: 520, minHeight: 350)
    }
}

final class DashboardData: ObservableObject {
    @Published var rows: [Row]
    @Published var message: String?
    private var timer: Timer?
    private var refreshing = false

    init(rows: [Row], executable: String, config: String) {
        self.rows = rows
        timer = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in
            guard let self, !self.refreshing else { return }
            self.refreshing = true
            DispatchQueue.global(qos: .utility).async {
                let result = Result { () throws -> [Row] in
                    let process = Process()
                    let output = Pipe()
                    process.executableURL = URL(fileURLWithPath: executable)
                    process.arguments = ["--config", config, "--json"]
                    process.standardOutput = output
                    try process.run()
                    let bytes = output.fileHandleForReading.readDataToEndOfFile()
                    process.waitUntilExit()
                    guard process.terminationStatus == 0 else {
                        throw CocoaError(.fileReadUnknown)
                    }
                    return try JSONDecoder().decode([Row].self, from: bytes)
                }
                DispatchQueue.main.async {
                    self.refreshing = false
                    switch result {
                    case .success(let rows): self.rows = rows; self.message = nil
                    case .failure: self.message = "Refresh failed · showing last results"
                    }
                }
            }
        }
    }

    deinit { timer?.invalidate() }
}

func dashboardIcon() -> NSImage {
    let size = NSSize(width: 128, height: 128)
    return NSImage(size: size, flipped: false) { _ in
        let tile = NSBezierPath(roundedRect: NSRect(x: 8, y: 8, width: 112, height: 112), xRadius: 27, yRadius: 27)
        NSGradient(starting: NSColor(srgbRed: 0.18, green: 0.23, blue: 0.35, alpha: 1),
                   ending: NSColor(srgbRed: 0.06, green: 0.08, blue: 0.14, alpha: 1))?.draw(in: tile, angle: -90)
        let lights: [NSColor] = [.systemMint, .systemYellow, .systemPink]
        for (index, color) in lights.enumerated() {
            let y = CGFloat(85 - index * 24)
            color.setFill()
            NSBezierPath(ovalIn: NSRect(x: 31, y: y, width: 12, height: 12)).fill()
            NSColor.white.withAlphaComponent(index == 0 ? 0.88 : 0.55).setFill()
            NSBezierPath(roundedRect: NSRect(x: 52, y: y + 3, width: CGFloat(45 - index * 5), height: 6),
                         xRadius: 3, yRadius: 3).fill()
        }
        return true
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

do {
    let rows = try JSONDecoder().decode([Row].self, from: FileHandle.standardInput.readDataToEndOfFile())
    let env = ProcessInfo.processInfo.environment
    guard let executable = env["MAC_STATUS_REFRESH_EXE"], let config = env["MAC_STATUS_REFRESH_CONFIG"] else {
        throw CocoaError(.fileReadInvalidFileName)
    }
    let data = DashboardData(rows: rows, executable: executable, config: config)
    let app = NSApplication.shared
    let delegate = AppDelegate()
    app.delegate = delegate
    app.setActivationPolicy(.regular)
    app.applicationIconImage = dashboardIcon()
    app.appearance = NSAppearance(named: .darkAqua)
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1060, height: 660),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable],
                          backing: .buffered, defer: false)
    window.title = "Mac Status"
    window.isReleasedWhenClosed = false
    window.contentViewController = NSHostingController(rootView: Dashboard(data: data))
    // NSHostingController can reset the initial frame to its minimum fitting size.
    // Set the real launch size after attaching it, bounded by the usable screen.
    if let screen = NSScreen.main {
        let visible = screen.visibleFrame
        let width = min(1100, visible.width - 32)
        let height = min(640, visible.height - 32)
        window.setFrame(NSRect(x: visible.midX - width / 2, y: visible.midY - height / 2,
                               width: width, height: height), display: true)
    } else {
        window.setContentSize(NSSize(width: 1060, height: 620))
        window.center()
    }
    window.makeKeyAndOrderFront(nil)
    app.activate(ignoringOtherApps: true)
    withExtendedLifetime(delegate) { app.run() }
} catch {
    fputs("mac-status: \(error)\n", stderr)
    exit(1)
}
