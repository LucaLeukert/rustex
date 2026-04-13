use anyhow::Result;
use rustex_ir::{Field, Function, FunctionKind, IrPackage, Table, TypeNode, Visibility};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub path: String,
    pub contents: String,
}

pub fn generate(package: &IrPackage) -> Result<Vec<GeneratedFile>> {
    let mut files = Vec::new();
    files.push(GeneratedFile {
        path: "Cargo.toml".into(),
        contents: cargo_toml(package),
    });
    files.push(GeneratedFile {
        path: "lib.rs".into(),
        contents: lib_rs(),
    });
    files.push(GeneratedFile {
        path: "ids.rs".into(),
        contents: ids_rs(package),
    });
    files.push(GeneratedFile {
        path: "models.rs".into(),
        contents: models_rs(package),
    });
    files.push(GeneratedFile {
        path: "api.rs".into(),
        contents: api_rs(package),
    });
    Ok(files)
}

fn cargo_toml(package: &IrPackage) -> String {
    format!(
        "[package]\nname = \"{}-generated\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = {{ version = \"1\", features = [\"derive\"] }}\nserde_json = \"1\"\n",
        package.project.name
    )
}

fn lib_rs() -> String {
    "pub mod api;\npub mod ids;\npub mod models;\n".into()
}

fn ids_rs(package: &IrPackage) -> String {
    let mut seen = BTreeSet::new();
    let mut out = String::from("use serde::{Deserialize, Serialize};\n\n");
    for table in &package.tables {
        if seen.insert(table.name.clone()) {
            let id_name = format!("{}Id", pascal_case(&table.name));
            out.push_str("#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]\n");
            out.push_str(&format!("pub struct {id_name}(pub String);\n\n"));
        }
    }
    out
}

fn models_rs(package: &IrPackage) -> String {
    let mut out = String::from(
        "use serde::{Deserialize, Serialize};\nuse std::collections::BTreeMap;\nuse crate::ids::*;\n\n",
    );
    for table in &package.tables {
        render_table(table, &mut out);
    }
    out
}

fn render_table(table: &Table, out: &mut String) {
    if let TypeNode::Object { fields, .. } = &table.document_type {
        out.push_str("#[derive(Clone, Debug, Serialize, Deserialize)]\n");
        out.push_str(&format!("pub struct {} {{\n", table.doc_name));
        out.push_str(&format!(
            "    pub _id: {}Id,\n    pub _creation_time: f64,\n",
            pascal_case(&table.name)
        ));
        for field in fields {
            render_field(field, out);
        }
        out.push_str("}\n\n");
    }
}

fn render_field(field: &Field, out: &mut String) {
    let rust_name = snake_case(&field.name);
    let ty = rust_type(&field.r#type, field.required);
    if rust_name != field.name {
        out.push_str(&format!("    #[serde(rename = \"{}\")]\n", field.name));
    }
    if !field.required {
        out.push_str("    #[serde(skip_serializing_if = \"Option::is_none\")]\n");
    }
    out.push_str(&format!("    pub {rust_name}: {ty},\n"));
}

fn api_rs(package: &IrPackage) -> String {
    let mut out = String::from(
        "use serde::{Deserialize, Serialize};\nuse crate::ids::*;\nuse crate::models::*;\n\n",
    );
    for function in &package.functions {
        render_function(function, &mut out);
    }
    out
}

fn render_function(function: &Function, out: &mut String) {
    let base = pascal_case(&function.export_name);
    if let Some(args) = &function.args_type {
        out.push_str("#[derive(Clone, Debug, Serialize, Deserialize)]\n");
        out.push_str(&format!("pub struct {base}Args {{\n"));
        match args {
            TypeNode::Object { fields, .. } => {
                for field in fields {
                    render_field(field, out);
                }
            }
            other => {
                out.push_str(&format!("    pub value: {},\n", rust_type(other, true)));
            }
        }
        out.push_str("}\n\n");
    }
    let response_ty = function
        .returns_type
        .as_ref()
        .map(|t| rust_type(t, true))
        .unwrap_or_else(|| "serde_json::Value".into());
    out.push_str(&format!("pub type {base}Response = {response_ty};\n\n"));
}

fn rust_type(node: &TypeNode, required: bool) -> String {
    let base = match node {
        TypeNode::String => "String".into(),
        TypeNode::Float64 => "f64".into(),
        TypeNode::Int64 => "i64".into(),
        TypeNode::Boolean => "bool".into(),
        TypeNode::Null => "()".into(),
        TypeNode::Bytes => "Vec<u8>".into(),
        TypeNode::Any => "serde_json::Value".into(),
        TypeNode::LiteralString { .. } => "String".into(),
        TypeNode::LiteralNumber { .. } => "f64".into(),
        TypeNode::LiteralBoolean { .. } => "bool".into(),
        TypeNode::Id { table } => format!("{}Id", pascal_case(table)),
        TypeNode::Array { element } => format!("Vec<{}>", rust_type(element, true)),
        TypeNode::Record { value } => format!("BTreeMap<String, {}>", rust_type(value, true)),
        TypeNode::Object { .. } => "serde_json::Value".into(),
        TypeNode::Union { members } => {
            if members.iter().any(|m| matches!(m, TypeNode::Null)) && members.len() == 2 {
                let inner = members
                    .iter()
                    .find(|m| !matches!(m, TypeNode::Null))
                    .map(|m| rust_type(m, true))
                    .unwrap_or_else(|| "serde_json::Value".into());
                return format!("Option<{inner}>");
            }
            "serde_json::Value".into()
        }
        TypeNode::Unknown { .. } => "serde_json::Value".into(),
    };
    if required {
        base
    } else {
        format!("Option<{base}>")
    }
}

fn pascal_case(input: &str) -> String {
    input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn snake_case(input: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if ch.is_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[allow(dead_code)]
fn _function_group(function: &Function) -> &'static str {
    match (&function.visibility, &function.kind) {
        (Visibility::Public, FunctionKind::Query) => "queries",
        (Visibility::Public, FunctionKind::Mutation) => "mutations",
        (Visibility::Public, FunctionKind::Action) => "actions",
        (Visibility::Internal, _) => "internal",
    }
}
