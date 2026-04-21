// swift-tools-version: 5.10

import PackageDescription

let package = Package(
  name: "RustexSwiftExample",
  platforms: [
    .iOS(.v13),
    .macOS(.v10_15),
  ],
  dependencies: [
    .package(path: "RustexGenerated"),
  ],
  targets: [
    .executableTarget(
      name: "RustexSwiftExample",
      dependencies: [
        .product(name: "RustexGenerated", package: "RustexGenerated"),
      ]
    ),
  ]
)
