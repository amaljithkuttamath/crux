import SwiftUI

struct SessionRowView: View {
    let session: ActiveSession

    var body: some View {
        HStack(spacing: 10) {
            // Health grade badge
            Text(session.healthGrade)
                .font(.system(size: 13, weight: .bold, design: .rounded))
                .foregroundColor(Theme.gradeColor(session.healthGrade))
                .frame(width: 28, height: 28)
                .background(
                    RoundedRectangle(cornerRadius: 7)
                        .fill(Theme.gradeColor(session.healthGrade).opacity(0.15))
                )

            // Project + meta
            VStack(alignment: .leading, spacing: 2) {
                Text(session.project)
                    .font(.system(size: 13, weight: .medium))
                    .lineLimit(1)
                    .truncationMode(.tail)

                HStack(spacing: 4) {
                    Text(session.durationLabel)
                    Text("\u{00B7}")
                    Text(session.model)
                    Text(session.sourceLabel)
                        .font(.system(size: 9, weight: .semibold))
                        .padding(.horizontal, 4)
                        .padding(.vertical, 1)
                        .background(
                            RoundedRectangle(cornerRadius: 3)
                                .fill(session.source == "claude_code"
                                      ? Theme.claudeCode.opacity(0.15)
                                      : Theme.cursor.opacity(0.15))
                        )
                        .foregroundColor(session.source == "claude_code"
                                         ? Theme.claudeCode
                                         : Theme.cursor)
                }
                .font(.system(size: 11))
                .foregroundColor(.secondary)
            }

            Spacer()

            // Cost + context bar
            VStack(alignment: .trailing, spacing: 3) {
                Text(Theme.formatCost(session.cost))
                    .font(.system(size: 12, weight: .medium).monospacedDigit())
                    .foregroundColor(.secondary)

                // Context fill bar
                GeometryReader { geo in
                    ZStack(alignment: .leading) {
                        RoundedRectangle(cornerRadius: 1.5)
                            .fill(Color.primary.opacity(0.06))
                        RoundedRectangle(cornerRadius: 1.5)
                            .fill(Theme.contextColor(session.contextPercent))
                            .frame(width: geo.size.width * min(session.contextPercent / 100, 1))
                    }
                }
                .frame(width: 40, height: 3)

                Text("\(Int(session.contextPercent))% ctx")
                    .font(.system(size: 9))
                    .foregroundColor(Color(nsColor: .quaternaryLabelColor))
            }
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 10)
    }
}
