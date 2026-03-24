import SwiftUI
import AppKit

@main
struct CruxBarApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var delegate

    var body: some Scene {
        Settings { EmptyView() }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var popover: NSPopover!
    private let dataLoader = DataLoader()
    private let processManager = ProcessManager()
    private var updateTimer: Timer?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        processManager.start()

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        updateMenuBarText()

        if let button = statusItem.button {
            button.action = #selector(togglePopover)
            button.target = self
        }

        popover = NSPopover()
        popover.behavior = .transient
        popover.animates = true
        // Let SwiftUI size the popover naturally
        let hostingController = NSHostingController(
            rootView: PopoverView(data: dataLoader.data, isStale: dataLoader.isStale)
        )
        popover.contentViewController = hostingController

        updateTimer = Timer.scheduledTimer(withTimeInterval: 10, repeats: true) { [weak self] _ in
            self?.refresh()
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        processManager.stop()
        updateTimer?.invalidate()
    }

    @objc private func togglePopover() {
        guard let button = statusItem.button else { return }
        if popover.isShown {
            popover.performClose(nil)
        } else {
            // Update content right before showing
            updatePopoverContent()
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            NSApp.activate(ignoringOtherApps: true)
        }
    }

    private func refresh() {
        updateMenuBarText()
        // Only update popover content if it's currently showing
        if popover.isShown {
            updatePopoverContent()
        }
    }

    private func updateMenuBarText() {
        guard let button = statusItem.button else { return }

        if let data = dataLoader.data, !dataLoader.isStale {
            let cost = Theme.formatCost(data.today.totalCost)
            let count = data.activeSessions.count

            let title = NSMutableAttributedString()

            let icon = NSAttributedString(
                string: "\u{25C9} ",
                attributes: [.font: NSFont.systemFont(ofSize: 12)]
            )
            title.append(icon)

            let costStr = NSAttributedString(
                string: cost,
                attributes: [
                    .font: NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium)
                ]
            )
            title.append(costStr)

            if count > 0 {
                let dot = NSAttributedString(
                    string: " \u{25CF}",
                    attributes: [
                        .font: NSFont.systemFont(ofSize: 7),
                        .foregroundColor: NSColor.systemGreen
                    ]
                )
                title.append(dot)

                let countStr = NSAttributedString(
                    string: " \(count)",
                    attributes: [
                        .font: NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular),
                        .foregroundColor: NSColor.secondaryLabelColor
                    ]
                )
                title.append(countStr)
            }

            button.attributedTitle = title
        } else {
            button.title = "\u{25C9} --"
        }
    }

    private func updatePopoverContent() {
        let view = PopoverView(data: dataLoader.data, isStale: dataLoader.isStale)
        popover.contentViewController = NSHostingController(rootView: view)
    }
}
