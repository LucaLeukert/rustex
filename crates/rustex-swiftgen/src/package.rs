use rustex_ir::IrPackage;
use rustex_project::{SwiftPackageRequirement, SwiftTargetConfig};

pub fn package_swift(config: &SwiftTargetConfig) -> String {
    let dependency = dependency_requirement(&config.convex_dependency_requirement);
    format!(
        "// swift-tools-version: {}\n\nimport PackageDescription\n\nlet package = Package(\n  name: \"{}\",\n  platforms: [\n    .iOS(.v13),\n    .macOS(.v10_15),\n  ],\n  products: [\n    .library(name: \"{}\", targets: [\"{}\"]),\n  ],\n  dependencies: [\n    .package(url: \"{}\", {}),\n  ],\n  targets: [\n    .target(\n      name: \"{}\",\n      dependencies: [\n        .product(name: \"ConvexMobile\", package: \"convex-swift\"),\n      ]\n    ),\n    .target(\n      name: \"{}\",\n      dependencies: [\n        \"{}\",\n        .product(name: \"ConvexMobile\", package: \"convex-swift\"),\n      ]\n    ),\n  ]\n)\n",
        config.tools_version,
        config.package_name,
        config.product_name,
        config.module_name,
        config.convex_dependency_url,
        dependency,
        config.runtime_module_name,
        config.module_name,
        config.runtime_module_name,
    )
}

pub fn readme(package: &IrPackage, config: &SwiftTargetConfig) -> String {
    format!(
        "# {}\n\nGenerated Swift bindings for the Convex app `{}`.\n\nImport `{}` from your app target and create a `{}` with either a deployment URL or an existing `ConvexClient` / `ConvexClientWithAuth` instance.\n\n```swift\nimport {}\n\nlet client = {}(deploymentUrl: deploymentUrl)\n```\n",
        config.package_name,
        package.project.name,
        config.module_name,
        config.client_facade_name,
        config.module_name,
        config.client_facade_name,
    )
}

fn dependency_requirement(requirement: &SwiftPackageRequirement) -> String {
    match requirement {
        SwiftPackageRequirement::From { version } => format!("from: \"{version}\""),
        SwiftPackageRequirement::Branch { branch } => format!(".branch(\"{branch}\")"),
        SwiftPackageRequirement::Exact { version } => format!("exact: \"{version}\""),
    }
}
