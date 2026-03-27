import Foundation

final class ProcessManager {
    private var process: Process?

    static func findCruxBinary() -> String? {
        let candidates = [
            "/opt/homebrew/bin/crux",
            "/usr/local/bin/crux",
            "\(FileManager.default.homeDirectoryForCurrentUser.path)/.cargo/bin/crux",
        ]
        for path in candidates {
            if FileManager.default.isExecutableFile(atPath: path) {
                return path
            }
        }
        let which = Process()
        which.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        which.arguments = ["crux"]
        let pipe = Pipe()
        which.standardOutput = pipe
        try? which.run()
        which.waitUntilExit()
        let output = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if let output, !output.isEmpty, FileManager.default.isExecutableFile(atPath: output) {
            return output
        }
        return nil
    }

    func start() {
        guard let binary = Self.findCruxBinary() else {
            print("CruxBar: crux binary not found")
            return
        }

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: binary)
        proc.arguments = ["export-widget", "--watch"]
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice

        do {
            try proc.run()
            process = proc
        } catch {
            print("CruxBar: failed to start crux: \(error)")
        }
    }

    func stop() {
        guard let proc = process, proc.isRunning else { return }
        proc.terminate()
        process = nil
    }
}
