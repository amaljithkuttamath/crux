// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "CruxBar",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "CruxBar",
            path: "Sources/CruxBar"
        ),
    ]
)
