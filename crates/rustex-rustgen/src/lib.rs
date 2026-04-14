use anyhow::Result;
use rustex_project::RustexConfig;
use rustex_ir::{Field, Function, FunctionKind, IrPackage, Table, TypeNode};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub path: String,
    pub contents: String,
}

pub fn generate(package: &IrPackage, config: &RustexConfig) -> Result<Vec<GeneratedFile>> {
    Ok(vec![
        GeneratedFile {
            path: "Cargo.toml".into(),
            contents: cargo_toml(package),
        },
        GeneratedFile {
            path: "lib.rs".into(),
            contents: lib_rs(),
        },
        GeneratedFile {
            path: "ids.rs".into(),
            contents: ids_rs(package),
        },
        GeneratedFile {
            path: "models.rs".into(),
            contents: models_rs(package, config),
        },
        GeneratedFile {
            path: "api.rs".into(),
            contents: api_rs(package, config),
        },
    ])
}

fn cargo_toml(package: &IrPackage) -> String {
    let runtime_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rustex-runtime")
        .canonicalize()
        .expect("runtime crate path");
    format!(
        "[package]\nname = \"{}-generated\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"lib.rs\"\n\n[workspace]\n\n[dependencies]\nserde = {{ version = \"1\", features = [\"derive\"] }}\nserde_json = \"1\"\nrustex-runtime = {{ path = \"{}\" }}\n",
        package.project.name,
        runtime_path.display()
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

fn models_rs(package: &IrPackage, config: &RustexConfig) -> String {
    let mut generator = TypeGenerator::new(config);
    let mut out = String::from(
        "#![allow(unused_imports)]\nuse serde::{Deserialize, Serialize};\nuse std::collections::BTreeMap;\nuse crate::ids::*;\n\n",
    );
    for table in &package.tables {
        render_table(table, &mut generator);
    }
    out.push_str(&generator.finish());
    out
}

fn render_table(table: &Table, generator: &mut TypeGenerator) {
    if let TypeNode::Object { fields, .. } = &table.document_type {
        let mut struct_fields = vec![
            RenderedField {
                rust_name: "_id".into(),
                original_name: "_id".into(),
                ty: format!("{}Id", pascal_case(&table.name)),
                required: true,
            },
            RenderedField {
                rust_name: "_creation_time".into(),
                original_name: "_creation_time".into(),
                ty: "f64".into(),
                required: true,
            },
        ];
        struct_fields.extend(generator.render_fields(fields, &table.doc_name));
        generator.push_struct_named(&table.doc_name, &struct_fields);
    }
}

fn api_rs(package: &IrPackage, config: &RustexConfig) -> String {
    let mut out = String::from(
        "#![allow(unused_imports)]\nuse serde::{Deserialize, Serialize};\nuse std::collections::BTreeMap;\nuse crate::ids::*;\nuse crate::models::*;\nuse rustex_runtime::{ActionSpec, FunctionSpec, MutationSpec, QuerySpec};\n\n",
    );

    let mut grouped: BTreeMap<String, Vec<&Function>> = BTreeMap::new();
    for function in &package.functions {
        grouped
            .entry(function.module_path.clone())
            .or_default()
            .push(function);
    }

    for (module_path, functions) in grouped {
        let mut generator = TypeGenerator::new_with_indent("    ", config);
        out.push_str(&format!("pub mod {} {{\n", module_ident(&module_path)));
        out.push_str("    use super::*;\n\n");
        for function in functions {
            render_function(function, &mut generator);
        }
        out.push_str(&generator.finish());
        out.push_str("}\n\n");
    }

    out
}

fn render_function(function: &Function, generator: &mut TypeGenerator) {
    let base = pascal_case(&function.export_name);
    let args_ty = format!("{base}Args");
    let output_ty = format!("{base}Response");

    match &function.args_type {
        Some(TypeNode::Object { fields, .. }) => {
            let rendered = generator.render_fields(fields, &args_ty);
            generator.push_struct_named(&args_ty, &rendered);
        }
        Some(other) => {
            let ty = generator.render_type(other, true, &args_ty);
            generator.push_alias_named(&args_ty, &ty);
        }
        None => generator.push_alias_named(&args_ty, "()"),
    }

    match &function.returns_type {
        Some(node) => match node {
            TypeNode::Object { fields, .. } => {
                let rendered = generator.render_fields(fields, &output_ty);
                generator.push_struct_named(&output_ty, &rendered);
            }
            _ => {
                let ty = generator.render_type(node, true, &output_ty);
                generator.push_alias_named(&output_ty, &ty);
            }
        },
        None => generator.push_alias_named(&output_ty, "()"),
    }

    generator.push_raw(&format!(
        "#[derive(Clone, Copy, Debug, Default)]\npub struct {base};\n\n"
    ));
    generator.push_raw(&format!(
        "pub fn {}() -> {base} {{\n    {base}\n}}\n\n",
        snake_case(&function.export_name)
    ));
    generator.push_raw(&format!("impl FunctionSpec for {base} {{\n"));
    generator.push_raw(&format!("    type Args = {args_ty};\n"));
    generator.push_raw(&format!("    type Output = {output_ty};\n"));
    generator.push_raw(&format!(
        "    const PATH: &'static str = \"{}\";\n",
        function.canonical_path
    ));
    generator.push_raw("}\n");

    match function.kind {
        FunctionKind::Query => generator.push_raw(&format!("impl QuerySpec for {base} {{}}\n\n")),
        FunctionKind::Mutation => {
            generator.push_raw(&format!("impl MutationSpec for {base} {{}}\n\n"))
        }
        FunctionKind::Action => {
            generator.push_raw(&format!("impl ActionSpec for {base} {{}}\n\n"))
        }
    }
}

#[derive(Debug, Clone)]
struct RenderedField {
    rust_name: String,
    original_name: String,
    ty: String,
    required: bool,
}

struct TypeGenerator {
    indent: &'static str,
    items: Vec<String>,
    used_names: BTreeSet<String>,
    derives: Vec<String>,
    attributes: Vec<String>,
}

impl TypeGenerator {
    fn new(config: &RustexConfig) -> Self {
        Self {
            indent: "",
            items: Vec::new(),
            used_names: BTreeSet::new(),
            derives: config.custom_derives.clone(),
            attributes: config.custom_attributes.clone(),
        }
    }

    fn new_with_indent(indent: &'static str, config: &RustexConfig) -> Self {
        Self {
            indent,
            items: Vec::new(),
            used_names: BTreeSet::new(),
            derives: config.custom_derives.clone(),
            attributes: config.custom_attributes.clone(),
        }
    }

    fn finish(self) -> String {
        self.items.concat()
    }

    fn push_raw(&mut self, raw: &str) {
        for line in raw.lines() {
            self.items.push(format!("{}{}\n", self.indent, line));
        }
    }

    fn push_alias_named(&mut self, name: &str, ty: &str) {
        let name = self.claim_name(name);
        self.items
            .push(format!("{}pub type {name} = {ty};\n\n", self.indent));
    }

    fn push_struct_named(&mut self, name: &str, fields: &[RenderedField]) {
        let name = self.claim_name(name);
        self.push_type_header(true);
        self.items
            .push(format!("{}pub struct {name} {{\n", self.indent));
        for field in fields {
            if field.rust_name != field.original_name {
                self.items.push(format!(
                    "{}    #[serde(rename = \"{}\")]\n",
                    self.indent, field.original_name
                ));
            }
            if !field.required {
                self.items.push(format!(
                    "{}    #[serde(skip_serializing_if = \"Option::is_none\")]\n",
                    self.indent
                ));
            }
            self.items.push(format!(
                "{}    pub {}: {},\n",
                self.indent, field.rust_name, field.ty
            ));
        }
        self.items.push(format!("{}}}\n\n", self.indent));
    }

    fn push_literal_enum_named(&mut self, name: &str, values: &[String]) -> String {
        let name = self.claim_name(name);
        self.push_type_header(false);
        self.items
            .push(format!("{}pub enum {name} {{\n", self.indent));
        let mut used_variants = BTreeSet::new();
        for value in values {
            let base = sanitize_variant(value);
            let variant = dedupe_name(&mut used_variants, &base);
            self.items.push(format!(
                "{}    #[serde(rename = \"{}\")]\n",
                self.indent, value
            ));
            self.items
                .push(format!("{}    {},\n", self.indent, variant));
        }
        self.items.push(format!("{}}}\n\n", self.indent));
        name
    }

    fn push_discriminated_enum_named(
        &mut self,
        name: &str,
        tag: &str,
        variants: &[(String, Vec<RenderedField>)],
    ) -> String {
        let name = self.claim_name(name);
        self.push_type_header(false);
        self.items.push(format!(
            "{}#[serde(tag = \"{}\")]\n",
            self.indent, tag
        ));
        self.items
            .push(format!("{}pub enum {name} {{\n", self.indent));
        let mut used_variants = BTreeSet::new();
        for (value, fields) in variants {
            let variant = dedupe_name(&mut used_variants, &sanitize_variant(value));
            self.items.push(format!(
                "{}    #[serde(rename = \"{}\")]\n",
                self.indent, value
            ));
            if fields.is_empty() {
                self.items
                    .push(format!("{}    {},\n", self.indent, variant));
            } else {
                self.items
                    .push(format!("{}    {} {{\n", self.indent, variant));
                for field in fields {
                    if field.rust_name != field.original_name {
                        self.items.push(format!(
                            "{}        #[serde(rename = \"{}\")]\n",
                            self.indent, field.original_name
                        ));
                    }
                    if !field.required {
                        self.items.push(format!(
                            "{}        #[serde(skip_serializing_if = \"Option::is_none\")]\n",
                            self.indent
                        ));
                    }
                    self.items.push(format!(
                        "{}        {}: {},\n",
                        self.indent, field.rust_name, field.ty
                    ));
                }
                self.items.push(format!("{}    }},\n", self.indent));
            }
        }
        self.items.push(format!("{}}}\n\n", self.indent));
        name
    }

    fn push_untagged_enum_named(
        &mut self,
        name: &str,
        variants: &[(String, Vec<RenderedField>)],
    ) -> String {
        let name = self.claim_name(name);
        self.push_type_header(false);
        self.items
            .push(format!("{}#[serde(untagged)]\n", self.indent));
        self.items
            .push(format!("{}pub enum {name} {{\n", self.indent));
        let mut used_variants = BTreeSet::new();
        for (variant_name, fields) in variants {
            let variant = dedupe_name(&mut used_variants, &sanitize_variant(variant_name));
            if fields.is_empty() {
                self.items
                    .push(format!("{}    {},\n", self.indent, variant));
            } else {
                self.items
                    .push(format!("{}    {} {{\n", self.indent, variant));
                for field in fields {
                    if field.rust_name != field.original_name {
                        self.items.push(format!(
                            "{}        #[serde(rename = \"{}\")]\n",
                            self.indent, field.original_name
                        ));
                    }
                    if !field.required {
                        self.items.push(format!(
                            "{}        #[serde(skip_serializing_if = \"Option::is_none\")]\n",
                            self.indent
                        ));
                    }
                    self.items.push(format!(
                        "{}        {}: {},\n",
                        self.indent, field.rust_name, field.ty
                    ));
                }
                self.items.push(format!("{}    }},\n", self.indent));
            }
        }
        self.items.push(format!("{}}}\n\n", self.indent));
        name
    }

    fn claim_name(&mut self, base: &str) -> String {
        let name = dedupe_name(&mut self.used_names, base);
        name
    }

    fn render_fields(&mut self, fields: &[Field], owner_name: &str) -> Vec<RenderedField> {
        let mut used = BTreeSet::new();
        fields
            .iter()
            .map(|field| {
                let rust_name = dedupe_name(&mut used, &snake_case(&field.name));
                let hint = format!("{owner_name}{}", pascal_case(&field.name));
                RenderedField {
                    rust_name,
                    original_name: field.name.clone(),
                    ty: self.render_type(&field.r#type, field.required, &hint),
                    required: field.required,
                }
            })
            .collect()
    }

    fn render_type(&mut self, node: &TypeNode, required: bool, hint: &str) -> String {
        let base = match node {
            TypeNode::String => "String".into(),
            TypeNode::Float64 => "f64".into(),
            TypeNode::Int64 => "i64".into(),
            TypeNode::Boolean => "bool".into(),
            TypeNode::Null => "()".into(),
            TypeNode::Bytes => "Vec<u8>".into(),
            TypeNode::Any => "serde_json::Value".into(),
            TypeNode::LiteralString { value } => {
                let enum_name = self.push_literal_enum_named(hint, std::slice::from_ref(value));
                enum_name
            }
            TypeNode::LiteralNumber { .. } => "f64".into(),
            TypeNode::LiteralBoolean { .. } => "bool".into(),
            TypeNode::Id { table } => format!("{}Id", pascal_case(table)),
            TypeNode::Array { element } => {
                let inner = self.render_type(element, true, &format!("{hint}Item"));
                format!("Vec<{inner}>")
            }
            TypeNode::Record { value } => {
                let inner = self.render_type(value, true, &format!("{hint}Value"));
                format!("BTreeMap<String, {inner}>")
            }
            TypeNode::Object { fields, .. } => {
                let struct_name = self.claim_name(hint);
                let rendered = self.render_fields(fields, &struct_name);
                self.push_struct_body(&struct_name, &rendered);
                struct_name
            }
            TypeNode::Union { members } => self.render_union(members, hint),
            TypeNode::Unknown { .. } => "serde_json::Value".into(),
        };

        if required {
            base
        } else {
            format!("Option<{base}>")
        }
    }

    fn render_union(&mut self, members: &[TypeNode], hint: &str) -> String {
        if let Some(non_null) = optional_member(members) {
            let inner = self.render_type(non_null, true, hint);
            return format!("Option<{inner}>");
        }

        if let Some(literals) = literal_string_union(members) {
            return self.push_literal_enum_named(hint, &literals);
        }

        if let Some((tag, variants)) = discriminated_union_members(members) {
            let rendered_variants = variants
                .into_iter()
                .map(|(value, fields)| {
                    let rendered = self.render_fields(&fields, &format!("{hint}{}", sanitize_variant(&value)));
                    (value, rendered)
                })
                .collect::<Vec<_>>();
            return self.push_discriminated_enum_named(hint, &tag, &rendered_variants);
        }

        if let Some(variants) = object_union_members(members) {
            let rendered_variants = variants
                .into_iter()
                .enumerate()
                .map(|(index, fields)| {
                    let variant_name = object_union_variant_name(&fields, index);
                    let rendered =
                        self.render_fields(&fields, &format!("{hint}Variant{}", index + 1));
                    (variant_name, rendered)
                })
                .collect::<Vec<_>>();
            return self.push_untagged_enum_named(hint, &rendered_variants);
        }

        "serde_json::Value".into()
    }

    fn push_struct_body(&mut self, name: &str, fields: &[RenderedField]) {
        self.push_type_header(true);
        self.items
            .push(format!("{}pub struct {name} {{\n", self.indent));
        for field in fields {
            if field.rust_name != field.original_name {
                self.items.push(format!(
                    "{}    #[serde(rename = \"{}\")]\n",
                    self.indent, field.original_name
                ));
            }
            if !field.required {
                self.items.push(format!(
                    "{}    #[serde(skip_serializing_if = \"Option::is_none\")]\n",
                    self.indent
                ));
            }
            self.items.push(format!(
                "{}    pub {}: {},\n",
                self.indent, field.rust_name, field.ty
            ));
        }
        self.items.push(format!("{}}}\n\n", self.indent));
    }

    fn push_type_header(&mut self, _is_struct: bool) {
        let mut derives = vec!["Clone", "Debug", "Serialize", "Deserialize", "PartialEq"];
        for derive in &self.derives {
            derives.push(derive);
        }
        self.items.push(format!(
            "{}#[derive({})]\n",
            self.indent,
            derives.join(", ")
        ));
        for attribute in &self.attributes {
            self.items
                .push(format!("{}#[{}]\n", self.indent, attribute));
        }
    }
}

fn optional_member(members: &[TypeNode]) -> Option<&TypeNode> {
    if members.len() == 2 && members.iter().any(|member| matches!(member, TypeNode::Null)) {
        members
            .iter()
            .find(|member| !matches!(member, TypeNode::Null))
    } else {
        None
    }
}

fn literal_string_union(members: &[TypeNode]) -> Option<Vec<String>> {
    let mut values = Vec::new();
    for member in members {
        if let TypeNode::LiteralString { value } = member {
            values.push(value.clone());
        } else {
            return None;
        }
    }
    if values.is_empty() { None } else { Some(values) }
}

fn discriminated_union_members(
    members: &[TypeNode],
) -> Option<(String, Vec<(String, Vec<Field>)>)> {
    let object_members = members
        .iter()
        .map(|member| match member {
            TypeNode::Object { fields, .. } => Some(fields.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    let candidate_tags = object_members
        .first()?
        .iter()
        .filter_map(|field| match &field.r#type {
            TypeNode::LiteralString { .. } => Some(field.name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    for tag in candidate_tags {
        let mut variants = Vec::new();
        let mut seen_values = BTreeSet::new();
        let mut valid = true;
        for fields in &object_members {
            let Some(discriminant) = fields.iter().find(|field| field.name == tag) else {
                valid = false;
                break;
            };
            let TypeNode::LiteralString { value } = &discriminant.r#type else {
                valid = false;
                break;
            };
            if !seen_values.insert(value.clone()) {
                valid = false;
                break;
            }
            let variant_fields = fields
                .iter()
                .filter(|field| field.name != tag)
                .cloned()
                .collect::<Vec<_>>();
            variants.push((value.clone(), variant_fields));
        }
        if valid {
            return Some((tag, variants));
        }
    }

    None
}

fn object_union_members(members: &[TypeNode]) -> Option<Vec<Vec<Field>>> {
    let mut object_members = members
        .iter()
        .map(|member| match member {
            TypeNode::Object { fields, .. } => Some(fields.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    object_members.sort_by(|left, right| {
        right
            .len()
            .cmp(&left.len())
            .then_with(|| object_union_variant_name(left, 0).cmp(&object_union_variant_name(right, 0)))
    });
    Some(object_members)
}

fn object_union_variant_name(fields: &[Field], index: usize) -> String {
    let joined = fields
        .iter()
        .map(|field| pascal_case(&field.name))
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>()
        .join("");
    if joined.is_empty() {
        format!("Variant{}", index + 1)
    } else {
        joined
    }
}

fn dedupe_name(used: &mut BTreeSet<String>, base: &str) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}{suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn sanitize_variant(input: &str) -> String {
    let base = pascal_case(input);
    match base.as_str() {
        "" => "Unknown".into(),
        "Self" | "Super" | "Crate" => format!("{base}Value"),
        _ if base
            .chars()
            .next()
            .map(|ch| ch.is_ascii_digit())
            .unwrap_or(false) =>
        {
            format!("V{base}")
        }
        _ => base,
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
    let mut prev_is_separator = true;
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            if ch.is_uppercase() && !prev_is_separator && !out.is_empty() {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
            prev_is_separator = false;
        } else if !prev_is_separator && !out.is_empty() {
            out.push('_');
            prev_is_separator = true;
        }
    }

    if out.is_empty() {
        "value".into()
    } else {
        out.trim_end_matches('_').to_string()
    }
}

fn module_ident(module_path: &str) -> String {
    module_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(snake_case)
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_object_unions_as_untagged_enums() {
        let mut generator = TypeGenerator::new(&RustexConfig::default());
        let ty = TypeNode::Union {
            members: vec![
                TypeNode::Object {
                    fields: vec![Field {
                        name: "error".into(),
                        required: true,
                        r#type: TypeNode::String,
                        doc: None,
                        source: None,
                    }],
                    open: false,
                },
                TypeNode::Object {
                    fields: vec![
                        Field {
                            name: "count".into(),
                            required: true,
                            r#type: TypeNode::Float64,
                            doc: None,
                            source: None,
                        },
                        Field {
                            name: "error".into(),
                            required: true,
                            r#type: TypeNode::String,
                            doc: None,
                            source: None,
                        },
                    ],
                    open: false,
                },
            ],
        };

        let rendered = generator.render_type(&ty, true, "MultiReturnDemoResponse");
        let output = generator.finish();

        assert_eq!(rendered, "MultiReturnDemoResponse");
        assert!(output.contains("#[serde(untagged)]"));
        assert!(output.contains("pub enum MultiReturnDemoResponse"));
        assert!(output.contains("CountError {"));
        assert!(output.contains("count: f64"));
        assert!(output.contains("Error {"));
        assert!(output.find("CountError {") < output.find("Error {"));
    }

    #[test]
    fn renders_short_unique_nested_names_for_object_union_variants() {
        let mut generator = TypeGenerator::new(&RustexConfig::default());
        let ty = TypeNode::Union {
            members: vec![
                TypeNode::Object {
                    fields: vec![Field {
                        name: "error".into(),
                        required: true,
                        r#type: TypeNode::String,
                        doc: None,
                        source: None,
                    }],
                    open: false,
                },
                TypeNode::Object {
                    fields: vec![
                        Field {
                            name: "messages".into(),
                            required: true,
                            r#type: TypeNode::Array {
                                element: Box::new(TypeNode::Object {
                                    fields: vec![Field {
                                        name: "body".into(),
                                        required: true,
                                        r#type: TypeNode::String,
                                        doc: None,
                                        source: None,
                                    }],
                                    open: false,
                                }),
                            },
                            doc: None,
                            source: None,
                        },
                        Field {
                            name: "count".into(),
                            required: true,
                            r#type: TypeNode::Float64,
                            doc: None,
                            source: None,
                        },
                        Field {
                            name: "error".into(),
                            required: true,
                            r#type: TypeNode::String,
                            doc: None,
                            source: None,
                        },
                    ],
                    open: false,
                },
            ],
        };

        generator.render_type(&ty, true, "MultiReturnDemoResponse");
        let output = generator.finish();

        assert!(output.contains("MultiReturnDemoResponseVariant1MessagesItem"));
        assert!(!output.contains("MultiReturnDemoResponseMessagesCountErrorMessagesItem"));
    }
}
