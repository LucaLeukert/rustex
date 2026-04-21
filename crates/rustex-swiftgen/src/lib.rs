mod api;
mod naming;
mod package;
mod runtime;
mod types;

use anyhow::Result;
use rustex_ir::IrPackage;
use rustex_project::RustexConfig;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub path: String,
    pub contents: String,
}

pub fn generate(package: &IrPackage, config: &RustexConfig) -> Result<Vec<GeneratedFile>> {
    let _span = tracing::info_span!(
        "rustex_swiftgen.generate",
        package = %package.project.name,
        tables = package.tables.len(),
        functions = package.functions.len()
    )
    .entered();
    debug!("rendering Swift bindings");

    let swift = &config.swift;
    let source_prefix = if swift.generate_package {
        format!("Sources/{}/", swift.module_name)
    } else {
        String::new()
    };
    let runtime_prefix = if swift.generate_package {
        format!("Sources/{}/", swift.runtime_module_name)
    } else {
        String::new()
    };

    let mut files = Vec::new();
    if swift.generate_package {
        files.push(GeneratedFile {
            path: "Package.swift".into(),
            contents: package::package_swift(swift),
        });
        files.push(GeneratedFile {
            path: "README.md".into(),
            contents: package::readme(package, swift),
        });
    }

    if swift.bundle_runtime {
        for (name, contents) in runtime::runtime_files(swift) {
            files.push(GeneratedFile {
                path: format!("{runtime_prefix}{name}"),
                contents,
            });
        }
    }

    files.push(GeneratedFile {
        path: format!("{source_prefix}RustexIds.swift"),
        contents: types::ids_swift(package, swift),
    });
    files.push(GeneratedFile {
        path: format!("{source_prefix}RustexModels.swift"),
        contents: types::models_swift(package, swift),
    });
    files.push(GeneratedFile {
        path: format!("{source_prefix}RustexAPI.swift"),
        contents: api::api_swift(package, swift),
    });

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustex_ir::{
        CapabilityFlags, ContractProvenance, Field, Function, FunctionKind, IrPackage,
        ManifestMeta, ProjectInfo, Table, TypeNode, Visibility,
    };
    use rustex_project::RustexConfig;

    fn fixture_package() -> IrPackage {
        IrPackage {
            project: ProjectInfo {
                name: "fixture".into(),
                root: ".".into(),
                convex_root: "convex".into(),
                convex_version: None,
                generated_metadata_present: true,
                discovered_convex_roots: Vec::new(),
                component_roots: Vec::new(),
            },
            tables: vec![Table {
                name: "messages".into(),
                doc_name: "MessagesDoc".into(),
                document_type: TypeNode::object(vec![
                    Field {
                        name: "body".into(),
                        required: true,
                        r#type: TypeNode::String,
                        doc: None,
                        source: None,
                    },
                    Field {
                        name: "score".into(),
                        required: false,
                        r#type: TypeNode::Float64,
                        doc: None,
                        source: None,
                    },
                    Field {
                        name: "in".into(),
                        required: true,
                        r#type: TypeNode::String,
                        doc: None,
                        source: None,
                    },
                ]),
                source: None,
            }],
            functions: vec![Function {
                canonical_path: "messages:add".into(),
                module_path: "messages".into(),
                export_name: "add".into(),
                component_path: None,
                visibility: Visibility::Public,
                kind: FunctionKind::Mutation,
                args_type: Some(TypeNode::object(vec![
                    Field {
                        name: "body".into(),
                        required: true,
                        r#type: TypeNode::String,
                        doc: None,
                        source: None,
                    },
                    Field {
                        name: "count".into(),
                        required: true,
                        r#type: TypeNode::Int64,
                        doc: None,
                        source: None,
                    },
                ])),
                returns_type: Some(TypeNode::Id {
                    table: "messages".into(),
                }),
                contract_provenance: ContractProvenance::Validator,
                source: None,
            }],
            named_types: Vec::new(),
            constraints: Vec::new(),
            capabilities: CapabilityFlags::default(),
            source_inventory: Vec::new(),
            diagnostics: Vec::new(),
            manifest_meta: ManifestMeta {
                rustex_version: "0.1.0".into(),
                manifest_version: 1,
                input_hash: "hash".into(),
            },
        }
    }

    #[test]
    fn generates_runtime_and_package_targets() {
        let files = generate(&fixture_package(), &RustexConfig::default()).expect("generate");
        let package = files
            .iter()
            .find(|file| file.path == "Package.swift")
            .expect("Package.swift");
        assert!(package.contents.contains("RustexRuntime"));
        assert!(package.contents.contains("RustexGenerated"));

        let runtime = files
            .iter()
            .find(|file| file.path.ends_with("RustexClient.swift"))
            .expect("runtime client");
        assert!(runtime.contents.contains("import Combine"));
        assert!(runtime.contents.contains("import ConvexMobile"));
        assert!(runtime.contents.contains("public final class RustexClient"));
        assert!(runtime.contents.contains("watchWebSocketState"));
    }

    #[test]
    fn generates_swift_models_with_convex_wrappers_and_coding_keys() {
        let files = generate(&fixture_package(), &RustexConfig::default()).expect("generate");
        let models = files
            .iter()
            .find(|file| file.path.ends_with("RustexModels.swift"))
            .expect("models");
        assert!(models.contents.contains("public struct MessagesDoc"));
        assert!(models.contents.contains("@OptionalConvexFloat"));
        assert!(models.contents.contains("public var score: Double?"));
        assert!(models.contents.contains("case in_ = \"in\""));
    }

    #[test]
    fn generates_api_specs_and_args() {
        let files = generate(&fixture_package(), &RustexConfig::default()).expect("generate");
        let api = files
            .iter()
            .find(|file| file.path.ends_with("RustexAPI.swift"))
            .expect("api");
        assert!(api.contents.contains("public enum API"));
        assert!(api.contents.contains("public enum Messages"));
        assert!(api.contents.contains("public enum Add: RustexMutationSpec"));
        assert!(
            api.contents
                .contains("public typealias Output = MessagesId")
        );
        assert!(
            api.contents
                .contains("public static let path = \"messages:add\"")
        );
        assert!(
            api.contents
                .contains("\"count\": try count.rustexConvexValue()")
        );
    }
}
