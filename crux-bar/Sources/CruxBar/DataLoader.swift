import Foundation
import Combine

@Observable
final class DataLoader {
    var data: WidgetData?
    var isStale: Bool = false

    private var timer: Timer?
    private let jsonPath: URL

    init() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        jsonPath = home.appendingPathComponent(".cache/crux/widget.json")
        load()
        startTimer()
    }

    deinit {
        timer?.invalidate()
    }

    private func startTimer() {
        timer = Timer.scheduledTimer(withTimeInterval: 10, repeats: true) { [weak self] _ in
            self?.load()
        }
    }

    private func load() {
        guard FileManager.default.fileExists(atPath: jsonPath.path) else {
            data = nil
            isStale = true
            return
        }

        guard let raw = try? Data(contentsOf: jsonPath) else {
            data = nil
            isStale = true
            return
        }

        let decoder = JSONDecoder()
        guard let parsed = try? decoder.decode(WidgetData.self, from: raw) else {
            data = nil
            isStale = true
            return
        }

        // Staleness check: if generated_at is more than 5 minutes old, mark stale
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let generatedDate = formatter.date(from: parsed.generatedAt) {
            isStale = Date().timeIntervalSince(generatedDate) > 300
        } else {
            formatter.formatOptions = [.withInternetDateTime]
            if let generatedDate = formatter.date(from: parsed.generatedAt) {
                isStale = Date().timeIntervalSince(generatedDate) > 300
            } else {
                isStale = true
            }
        }

        data = isStale ? nil : parsed
    }
}
