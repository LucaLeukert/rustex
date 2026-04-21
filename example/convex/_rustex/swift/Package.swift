// swift-tools-version: 5.10

import PackageDescription

let package = Package(
  name: "RustexGenerated",
  platforms: [
    .iOS(.v13),
    .macOS(.v10_15),
  ],
  products: [
    .library(name: "RustexGenerated", targets: ["RustexGenerated"]),
  ],
  dependencies: [
    .package(url: "https://github.com/get-convex/convex-swift", from: "0.8.1"),
  ],
  targets: [
    .target(
      name: "RustexRuntime",
      dependencies: [
        .product(name: "ConvexMobile", package: "convex-swift"),
      ]
    ),
    .target(
      name: "RustexGenerated",
      dependencies: [
        "RustexRuntime",
        .product(name: "ConvexMobile", package: "convex-swift"),
      ]
    ),
  ]
)
