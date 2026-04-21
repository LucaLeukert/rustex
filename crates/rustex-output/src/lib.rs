use anyhow::Result;
use camino::Utf8Path;
use rustex_ir::{ConstraintKind, IrPackage, NamedType, Origin, TypeNode};
use rustex_rustgen::GeneratedFile as RustGeneratedFile;
use rustex_swiftgen::GeneratedFile as SwiftGeneratedFile;
use serde_json::{Map, Value, json};
use tracing::info;

pub fn write_ir(package: &IrPackage, out_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(
        out_dir.join("rustex.ir.json"),
        serde_json::to_string_pretty(package)?,
    )?;
    info!(
        "IR document written to {}",
        display_path(out_dir, &package.project.root)
    );
    Ok(())
}

pub fn write_manifest(package: &IrPackage, out_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(
        out_dir.join("rustex.manifest.json"),
        serde_json::to_string_pretty(&package.manifest_meta)?,
    )?;
    info!(
        "Manifest document written to {}",
        display_path(out_dir, &package.project.root)
    );
    Ok(())
}

pub fn write_diagnostics(package: &IrPackage, out_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(
        out_dir.join("rustex.diagnostics.json"),
        serde_json::to_string_pretty(&package.diagnostics)?,
    )?;
    info!(
        "Diagnostics document written to {}",
        display_path(out_dir, &package.project.root)
    );
    Ok(())
}

pub fn write_json_schema(package: &IrPackage, out_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(
        out_dir.join("rustex.schema.json"),
        serde_json::to_string_pretty(&json_schema_document(package))?,
    )?;
    info!(
        "JSON schema document written to {}",
        display_path(out_dir, &package.project.root)
    );
    Ok(())
}

pub fn write_openapi(package: &IrPackage, out_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(
        out_dir.join("rustex.openapi.json"),
        serde_json::to_string_pretty(&openapi_document(package))?,
    )?;
    info!(
        "OpenAPI document written to {}",
        display_path(out_dir, &package.project.root)
    );
    Ok(())
}

pub fn write_source_map(package: &IrPackage, out_dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(
        out_dir.join("rustex.source_map.json"),
        serde_json::to_string_pretty(&source_map_document(package))?,
    )?;
    info!(
        "Source map document written to {}",
        display_path(out_dir, &package.project.root)
    );
    Ok(())
}

pub fn write_rust(
    files: &[RustGeneratedFile],
    out_dir: &Utf8Path,
    project_root: &Utf8Path,
) -> Result<()> {
    write_generated_files(
        files
            .iter()
            .map(|file| (file.path.as_str(), file.contents.as_str())),
        out_dir,
        project_root,
        "Rust",
    )
}

pub fn write_swift(
    files: &[SwiftGeneratedFile],
    out_dir: &Utf8Path,
    project_root: &Utf8Path,
) -> Result<()> {
    write_generated_files(
        files
            .iter()
            .map(|file| (file.path.as_str(), file.contents.as_str())),
        out_dir,
        project_root,
        "Swift",
    )
}

fn write_generated_files<'a>(
    files: impl Iterator<Item = (&'a str, &'a str)>,
    out_dir: &Utf8Path,
    project_root: &Utf8Path,
    label: &str,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    for (file_path, contents) in files {
        let path = out_dir.join(file_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
    }
    info!(
        "{label} files written to {}",
        display_path(out_dir, project_root)
    );
    Ok(())
}

fn display_path(path: &Utf8Path, project_root: &Utf8Path) -> String {
    path.strip_prefix(project_root)
        .map(Utf8Path::to_string)
        .unwrap_or_else(|_| path.to_string())
}

pub fn source_map_document(package: &IrPackage) -> Value {
    let mut entries = Vec::new();
    for table in &package.tables {
        if let Some(source) = &table.source {
            entries.push(source_map_entry(
                "models.rs",
                &table.doc_name,
                source,
                format!("table.{}", table.name),
            ));
        }
    }
    for function in &package.functions {
        if let Some(source) = &function.source {
            entries.push(source_map_entry(
                "api.rs",
                &format!("{}::{}", function.module_path, function.export_name),
                source,
                format!("function.{}", function.canonical_path),
            ));
        }
    }
    for named_type in &package.named_types {
        if let Some(source) = &named_type.source {
            let generated_file = if named_type.origin_symbol.starts_with("table.") {
                "models.rs"
            } else {
                "api.rs"
            };
            entries.push(source_map_entry(
                generated_file,
                &named_type.suggested_name,
                source,
                named_type.key.clone(),
            ));
        }
    }
    Value::Array(entries)
}

fn source_map_entry(
    generated_file: &str,
    generated_symbol: &str,
    source: &Origin,
    ir_key: String,
) -> Value {
    json!({
        "generated_file": generated_file,
        "generated_symbol": generated_symbol,
        "ir_key": ir_key,
        "source": {
            "file": source.file,
            "line": source.line,
            "column": source.column
        }
    })
}

pub fn json_schema_document(package: &IrPackage) -> Value {
    let mut defs = Map::new();
    for named_type in &package.named_types {
        defs.insert(
            named_type.suggested_name.clone(),
            schema_for_named_type(named_type),
        );
    }

    let functions = package
        .functions
        .iter()
        .map(|function| {
            json!({
                "path": function.canonical_path,
                "args": function.args_type.as_ref().map(schema_for_type).unwrap_or(json!({"type":"object"})),
                "returns": function.returns_type.as_ref().map(schema_for_type).unwrap_or(json!({})),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": package.project.name,
        "type": "object",
        "$defs": defs,
        "properties": {
            "tables": Value::Array(package.tables.iter().map(|table| {
                json!({
                    "name": table.name,
                    "document": schema_for_type(&table.document_type)
                })
            }).collect()),
            "functions": Value::Array(functions)
        }
    })
}

fn schema_for_named_type(named_type: &NamedType) -> Value {
    let mut schema = schema_for_type(&named_type.node);
    if let Value::Object(ref mut object) = schema {
        object.insert(
            "title".into(),
            Value::String(named_type.suggested_name.clone()),
        );
    }
    schema
}

fn schema_for_type(node: &TypeNode) -> Value {
    match node {
        TypeNode::String => json!({"type":"string"}),
        TypeNode::Float64 => json!({"type":"number"}),
        TypeNode::Int64 => json!({"type":"integer"}),
        TypeNode::Boolean => json!({"type":"boolean"}),
        TypeNode::Null => json!({"type":"null"}),
        TypeNode::Bytes => json!({"type":"string","contentEncoding":"base64"}),
        TypeNode::Any | TypeNode::Unknown { .. } => json!({}),
        TypeNode::LiteralString { value } => json!({"type":"string","const":value}),
        TypeNode::LiteralNumber { value } => json!({"type":"number","const":value}),
        TypeNode::LiteralBoolean { value } => json!({"type":"boolean","const":value}),
        TypeNode::Id { table } => json!({"type":"string","x-rustex-id-table":table}),
        TypeNode::Array { element } => json!({"type":"array","items":schema_for_type(element)}),
        TypeNode::Record { value } => {
            json!({"type":"object","additionalProperties":schema_for_type(value)})
        }
        TypeNode::Object { fields, .. } => {
            let properties = fields
                .iter()
                .map(|field| (field.name.clone(), schema_for_type(&field.r#type)))
                .collect::<Map<_, _>>();
            let required = fields
                .iter()
                .filter(|field| field.required)
                .map(|field| Value::String(field.name.clone()))
                .collect::<Vec<_>>();
            json!({
                "type":"object",
                "properties": properties,
                "required": required
            })
        }
        TypeNode::Union { members } => {
            json!({"oneOf": members.iter().map(schema_for_type).collect::<Vec<_>>()})
        }
    }
}

pub fn openapi_document(package: &IrPackage) -> Value {
    let mut paths = Map::new();
    for function in &package.functions {
        let operation_id = function.canonical_path.replace(':', "_");
        let method = if function.canonical_path.starts_with("http:") {
            "get"
        } else {
            "post"
        };
        let operation = json!({
            "operationId": operation_id,
            "x-rustex-kind": format!("{:?}", function.kind).to_lowercase(),
            "requestBody": {
                "content": {
                    "application/json": {
                        "schema": function.args_type.as_ref().map(schema_for_type).unwrap_or(json!({"type":"object"}))
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Successful response",
                    "content": {
                        "application/json": {
                            "schema": function.returns_type.as_ref().map(schema_for_type).unwrap_or(json!({}))
                        }
                    }
                }
            }
        });
        let mut path_item = Map::new();
        path_item.insert(method.to_string(), operation);
        paths.insert(
            format!("/{}", function.canonical_path.replace(':', "/")),
            Value::Object(path_item),
        );
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": package.project.name,
            "version": package.project.convex_version
        },
        "paths": paths,
        "x-rustex-capabilities": package.capabilities,
        "x-rustex-constraints": package.constraints.iter().map(|constraint| {
            json!({
                "path": constraint.path,
                "kind": match constraint.kind {
                    ConstraintKind::Literal => "literal",
                    ConstraintKind::Optional => "optional",
                    ConstraintKind::RecordValue => "record_value",
                    ConstraintKind::Discriminant => "discriminant",
                    ConstraintKind::IdentifierTable => "identifier_table",
                },
                "detail": constraint.detail
            })
        }).collect::<Vec<_>>()
    })
}
