// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "GooseMobileClient",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(name: "GooseMobileClient", targets: ["GooseMobileClient"]),
        .executable(name: "goose-mobile-demo", targets: ["GooseMobileDemo"]),
    ],
    targets: [
        .target(name: "GooseMobileClient"),
        .executableTarget(
            name: "GooseMobileDemo",
            dependencies: ["GooseMobileClient"]
        ),
        .testTarget(
            name: "GooseMobileClientTests",
            dependencies: ["GooseMobileClient"]
        ),
    ]
)
