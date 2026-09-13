// swift-tools-version: 5.9
import PackageDescription

// The package target contains platform-independent policy and storage helpers.
// The iOS application target is defined by Ichoi.xcodeproj and adds SwiftUI,
// AVFoundation, Security, and the generated CSIL client.
let package = Package(
    name: "IchoiCore",
    platforms: [.iOS(.v16), .macOS(.v13)],
    products: [.library(name: "IchoiCore", targets: ["IchoiCore"])],
    targets: [
        .target(name: "IchoiCore", path: "IchoiCore"),
        .testTarget(name: "IchoiCoreTests", dependencies: ["IchoiCore"], path: "Tests/IchoiCoreTests")
    ]
)
