import SwiftUI

struct SessionRowView: View {
    let session: ActiveSession

    var body: some View {
        HStack(spacing: 8) {
            // Health grade badge
            Text(session.healthGrade)
                .font(.system(size: 12, weight: .bold, design: .rounded))
                .foregroundColor(Theme.gradeColor(session.healthGrade))
                .frame(width: 24, height: 24)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(Theme.gradeColor(session.healthGrade).opacity(0.12))
                )

            // Project + meta
            VStack(alignment: .leading, spacing: 1) {
                Text(session.project)
                    .font(.system(size: 12, weight: .medium))
                    .lineLimit(1)
                    .truncationMode(.tail)

                HStack(spacing: 3) {
                    Text(session.durationLabel)
                    Text("\u{00B7}")
                    Text(session.model)
                }
                .font(.system(size: 10))
                .foregroundColor(Color(nsColor: .tertiaryLabelColor))
            }

            Spacer()

            // Cost + context
            VStack(alignment: .trailing, spacing: 2) {
                Text(Theme.formatCost(session.cost))
                    .font(.system(size: 11, weight: .medium).monospacedDigit())

                // Context as a compact inline indicator
                HStack(spacing: 3) {
                    // Tiny bar
                    ZStack(alignment: .leading) {
                        RoundedRectangle(cornerRadius: 1)
                            .fill(Color.primary.opacity(0.06))
                            .frame(width: 30, height: 2.5)
                        RoundedRectangle(cornerRadius: 1)
                            .fill(Theme.contextColor(session.contextPercent))
                            .frame(
                                width: 30 * min(session.contextPercent / 100, 1),
                                height: 2.5
                            )
                    }
                    Text("\(Int(session.contextPercent))%")
                        .font(.system(size: 9).monospacedDigit())
                        .foregroundColor(Color(nsColor: .quaternaryLabelColor))
                }
            }
        }
        .padding(.vertical, 5)
        .padding(.horizontal, 10)
    }
}
