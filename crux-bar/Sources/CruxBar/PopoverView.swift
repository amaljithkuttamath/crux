import SwiftUI

struct PopoverView: View {
    let data: WidgetData?
    let isStale: Bool

    var body: some View {
        VStack(spacing: 0) {
            if let data, !isStale {
                contentView(data)
            } else {
                emptyView
            }

            Divider()

            Button("Open Crux") {
                openCruxTUI()
            }
            .buttonStyle(.plain)
            .font(.system(size: 12, weight: .medium))
            .foregroundColor(.blue)
            .padding(.vertical, 8)
        }
        .frame(width: 300)
    }

    @ViewBuilder
    private func contentView(_ data: WidgetData) -> some View {
        VStack(spacing: 0) {
            // Hero: cost + burn rate
            HStack(alignment: .firstTextBaseline) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(Theme.formatCost(data.today.totalCost))
                        .font(.system(size: 30, weight: .bold).monospacedDigit())
                    Text("today")
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 1) {
                    Text(Theme.formatCost(data.today.burnRatePerHour) + "/hr")
                        .font(.system(size: 13, weight: .semibold).monospacedDigit())
                        .foregroundColor(.secondary)
                    Text("burn rate")
                        .font(.system(size: 10))
                        .foregroundColor(Color(nsColor: .quaternaryLabelColor))
                }
            }
            .padding(.horizontal, 14)
            .padding(.top, 14)
            .padding(.bottom, 12)

            // Source split bar
            VStack(spacing: 6) {
                GeometryReader { geo in
                    HStack(spacing: 2) {
                        let total = data.today.sources.claudeCode + data.today.sources.cursor
                        let ccFraction = total > 0 ? data.today.sources.claudeCode / total : 0.5
                        RoundedRectangle(cornerRadius: 2)
                            .fill(Theme.claudeCode)
                            .frame(width: max(geo.size.width * ccFraction, 2))
                        RoundedRectangle(cornerRadius: 2)
                            .fill(Theme.cursor)
                    }
                }
                .frame(height: 4)

                HStack {
                    Label {
                        Text("Claude Code " + Theme.formatCost(data.today.sources.claudeCode))
                    } icon: {
                        Circle().fill(Theme.claudeCode).frame(width: 6, height: 6)
                    }
                    Spacer()
                    Label {
                        Text("Cursor " + Theme.formatCost(data.today.sources.cursor))
                    } icon: {
                        Circle().fill(Theme.cursor).frame(width: 6, height: 6)
                    }
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            }
            .padding(.horizontal, 14)
            .padding(.bottom, 14)

            // Active sessions
            if !data.activeSessions.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("ACTIVE SESSIONS")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundColor(Color(nsColor: .tertiaryLabelColor))
                        .tracking(0.5)
                        .padding(.horizontal, 14)

                    VStack(spacing: 0) {
                        ForEach(Array(data.activeSessions.enumerated()), id: \.element.id) { index, session in
                            if index > 0 {
                                Divider().padding(.leading, 48)
                            }
                            SessionRowView(session: session)
                        }
                    }
                    .background(
                        RoundedRectangle(cornerRadius: 10)
                            .fill(Color.primary.opacity(0.04))
                    )
                    .padding(.horizontal, 8)
                }
                .padding(.bottom, 14)
            } else {
                Text("No active sessions")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
                    .padding(.vertical, 20)
            }
        }
    }

    private var emptyView: some View {
        VStack(spacing: 8) {
            Image(systemName: "chart.bar.xaxis")
                .font(.system(size: 28))
                .foregroundColor(.secondary)
            Text("No data")
                .font(.system(size: 14, weight: .medium))
            Text("Run crux to start")
                .font(.system(size: 12))
                .foregroundColor(.secondary)
        }
        .padding(.vertical, 30)
    }

    private func openCruxTUI() {
        let script = "tell application \"Terminal\" to do script \"crux\""
        if let appleScript = NSAppleScript(source: script) {
            var error: NSDictionary?
            appleScript.executeAndReturnError(&error)
        }
    }
}
