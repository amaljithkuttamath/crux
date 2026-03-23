import Foundation

struct WidgetData: Codable {
    let generatedAt: String
    let today: TodaySummary
    let activeSessions: [ActiveSession]

    enum CodingKeys: String, CodingKey {
        case generatedAt = "generated_at"
        case today
        case activeSessions = "active_sessions"
    }
}

struct TodaySummary: Codable {
    let totalCost: Double
    let burnRatePerHour: Double
    let sources: SourceCosts

    enum CodingKeys: String, CodingKey {
        case totalCost = "total_cost"
        case burnRatePerHour = "burn_rate_per_hour"
        case sources
    }
}

struct SourceCosts: Codable {
    let claudeCode: Double
    let cursor: Double

    enum CodingKeys: String, CodingKey {
        case claudeCode = "claude_code"
        case cursor
    }
}

struct ActiveSession: Codable, Identifiable {
    let sessionId: String
    let project: String
    let source: String
    let model: String
    let durationMinutes: Int
    let cost: Double
    let healthGrade: String
    let contextPercent: Double

    var id: String { sessionId }

    var sourceLabel: String {
        source == "claude_code" ? "CC" : "Cursor"
    }

    var durationLabel: String {
        if durationMinutes >= 60 {
            return "\(durationMinutes / 60)h\(durationMinutes % 60)m"
        }
        return "\(durationMinutes)m"
    }

    enum CodingKeys: String, CodingKey {
        case project, source, model, cost
        case sessionId = "session_id"
        case durationMinutes = "duration_minutes"
        case healthGrade = "health_grade"
        case contextPercent = "context_percent"
    }
}
