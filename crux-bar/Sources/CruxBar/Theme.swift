import SwiftUI

enum Theme {
    static func gradeColor(_ grade: String) -> Color {
        switch grade {
        case "A": return .green
        case "B": return .yellow
        case "C": return .orange
        case "D", "F": return .red
        default: return .secondary
        }
    }

    static let claudeCode: Color = .green
    static let cursor: Color = .blue

    static func contextColor(_ percent: Double) -> Color {
        if percent >= 85 { return .red }
        if percent >= 70 { return .orange }
        if percent >= 50 { return .yellow }
        return .green
    }

    static func formatCost(_ cost: Double) -> String {
        if cost >= 0.01 {
            return String(format: "$%.2f", cost)
        }
        return String(format: "$%.3f", cost)
    }
}
