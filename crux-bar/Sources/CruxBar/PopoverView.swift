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
        }
        .frame(width: 280)
    }

    // MARK: - Filters

    /// Filter out obviously stale sessions (>8 hours), sort by cost descending
    private func recentSessions(_ sessions: [ActiveSession]) -> [ActiveSession] {
        sessions
            .filter { $0.durationMinutes < 480 }
            .sorted { $0.cost > $1.cost }
    }

    // MARK: - Content

    @ViewBuilder
    private func contentView(_ data: WidgetData) -> some View {
        VStack(spacing: 0) {
            // -- Hero --
            heroSection(data)

            Divider().padding(.horizontal, 12)

            // -- Source split (only if both sources have data) --
            if data.today.sources.claudeCode > 0 && data.today.sources.cursor > 0 {
                sourceSection(data)
                Divider().padding(.horizontal, 12)
            }

            // -- Sessions --
            let recent = recentSessions(data.activeSessions)
            if !recent.isEmpty {
                sessionsSection(recent, totalActive: data.activeSessions.count)
            } else if !data.activeSessions.isEmpty {
                // Have sessions but all are stale
                idleSection(sessionCount: data.activeSessions.count)
            } else {
                idleSection(sessionCount: 0)
            }

            // -- Footer --
            footerSection
        }
    }

    // MARK: - Hero

    private func heroSection(_ data: WidgetData) -> some View {
        HStack(alignment: .lastTextBaseline) {
            // Cost
            Text(Theme.formatCost(data.today.totalCost))
                .font(.system(size: 24, weight: .semibold).monospacedDigit())

            Text("today")
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .padding(.bottom, 1)

            Spacer()

            // Burn rate
            if data.today.burnRatePerHour > 0.01 {
                Text(Theme.formatCost(data.today.burnRatePerHour) + "/hr")
                    .font(.system(size: 12, weight: .medium).monospacedDigit())
                    .foregroundColor(.secondary)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
    }

    // MARK: - Source Split

    private func sourceSection(_ data: WidgetData) -> some View {
        HStack(spacing: 12) {
            sourceChip(
                label: "Claude Code",
                cost: data.today.sources.claudeCode,
                color: Theme.claudeCode
            )
            sourceChip(
                label: "Cursor",
                cost: data.today.sources.cursor,
                color: Theme.cursor
            )
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    private func sourceChip(label: String, cost: Double, color: Color) -> some View {
        HStack(spacing: 5) {
            Circle()
                .fill(color)
                .frame(width: 6, height: 6)
            Text(label)
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            Text(Theme.formatCost(cost))
                .font(.system(size: 11, weight: .medium).monospacedDigit())
                .foregroundColor(.primary)
        }
    }

    // MARK: - Sessions

    private func sessionsSection(_ sessions: [ActiveSession], totalActive: Int) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            HStack {
                Text("ACTIVE")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundColor(Color(nsColor: .tertiaryLabelColor))
                    .tracking(0.5)

                Spacer()

                if totalActive > sessions.count {
                    Text("+\(totalActive - sessions.count) more")
                        .font(.system(size: 10))
                        .foregroundColor(Color(nsColor: .tertiaryLabelColor))
                }
            }
            .padding(.horizontal, 14)
            .padding(.top, 10)
            .padding(.bottom, 6)

            // Session rows (max 4)
            VStack(spacing: 0) {
                ForEach(Array(sessions.prefix(4).enumerated()), id: \.element.id) { index, session in
                    if index > 0 {
                        Divider()
                            .padding(.leading, 42)
                            .padding(.trailing, 8)
                    }
                    SessionRowView(session: session)
                }
            }
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(Color.primary.opacity(0.03))
            )
            .padding(.horizontal, 8)
            .padding(.bottom, 10)
        }
    }

    // MARK: - Idle

    private func idleSection(sessionCount: Int) -> some View {
        VStack(spacing: 4) {
            Text(sessionCount > 0 ? "No recent activity" : "No active sessions")
                .font(.system(size: 12))
                .foregroundColor(.secondary)
            if sessionCount > 0 {
                Text("\(sessionCount) stale sessions running")
                    .font(.system(size: 10))
                    .foregroundColor(Color(nsColor: .quaternaryLabelColor))
            }
        }
        .padding(.vertical, 16)
    }

    // MARK: - Footer

    private var footerSection: some View {
        VStack(spacing: 0) {
            Divider()
            Button(action: openCruxTUI) {
                Text("Open Crux")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(.blue)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 7)
            }
            .buttonStyle(.plain)
        }
    }

    // MARK: - Empty

    private var emptyView: some View {
        VStack(spacing: 6) {
            Image(systemName: "waveform.path.ecg")
                .font(.system(size: 24))
                .foregroundColor(Color(nsColor: .tertiaryLabelColor))
            Text("No data")
                .font(.system(size: 13, weight: .medium))
                .foregroundColor(.secondary)
            Text("Run crux to start monitoring")
                .font(.system(size: 11))
                .foregroundColor(Color(nsColor: .tertiaryLabelColor))
        }
        .padding(.vertical, 28)
    }

    // MARK: - Actions

    private func openCruxTUI() {
        guard let binary = ProcessManager.findCruxBinary() else { return }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        proc.arguments = ["-a", "Terminal", binary]
        try? proc.run()
    }
}
