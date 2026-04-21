use crate::naming::{camel_case, dedupe, enum_case, escape_keyword, pascal_case};
use rustex_ir::{Field, IrPackage, Table, TypeNode};
use rustex_project::SwiftTargetConfig;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
struct RenderedField {
    swift_name: String,
    original_name: String,
    ty: String,
    wrapper: Option<&'static str>,
}

pub fn ids_swift(package: &IrPackage, config: &SwiftTargetConfig) -> String {
    let access = config.access_level.as_swift();
    let mut seen = BTreeSet::new();
    let mut out = prelude(config);
    for table in &package.tables {
        if !seen.insert(table.name.clone()) {
            continue;
        }
        let name = format!("{}Id", pascal_case(&table.name));
        out.push_str(&format!(
            "{access} struct {name}: Codable, Hashable, ExpressibleByStringLiteral {{\n  {access} let rawValue: String\n\n  {access} init(_ rawValue: String) {{\n    self.rawValue = rawValue\n  }}\n\n  {access} init(stringLiteral value: String) {{\n    self.rawValue = value\n  }}\n\n  {access} init(from decoder: Decoder) throws {{\n    self.rawValue = try decoder.singleValueContainer().decode(String.self)\n  }}\n\n  {access} func encode(to encoder: Encoder) throws {{\n    var container = encoder.singleValueContainer()\n    try container.encode(rawValue)\n  }}\n}}\n\nextension {name}: ConvexEncodable {{}}\nextension {name}: RustexConvexValueConvertible {{\n  {access} func rustexConvexValue() throws -> ConvexEncodable? {{ rawValue }}\n}}\n\n"
        ));
    }
    out
}

pub fn models_swift(package: &IrPackage, config: &SwiftTargetConfig) -> String {
    let mut generator = TypeGenerator::new(config, Mode::Decode);
    let mut out = prelude(config);
    for table in &package.tables {
        render_table(table, &mut generator);
    }
    out.push_str(&generator.finish());
    out
}

pub(crate) fn render_args_type(
    node: Option<&TypeNode>,
    name: &str,
    generator: &mut TypeGenerator,
) -> String {
    match node {
        Some(TypeNode::Object { fields, .. }) => {
            let rendered = generator.render_fields(fields, name);
            generator.push_struct_named(name, &rendered, StructRole::Args);
            name.into()
        }
        Some(node) => generator.render_type(node, true, name),
        None => "RustexNoArgs".into(),
    }
}

pub(crate) fn render_output_type(
    node: Option<&TypeNode>,
    name: &str,
    generator: &mut TypeGenerator,
) -> String {
    match node {
        Some(TypeNode::Object { fields, .. }) => {
            let rendered = generator.render_fields(fields, name);
            generator.push_struct_named(name, &rendered, StructRole::Decode);
            name.into()
        }
        Some(TypeNode::Null) | None => "RustexVoid".into(),
        Some(node) => generator.render_type(node, true, name),
    }
}

pub(crate) fn api_type_generator(config: &SwiftTargetConfig) -> TypeGenerator {
    TypeGenerator::new(config, Mode::Args)
}

pub(crate) struct TypeGenerator {
    config: SwiftTargetConfig,
    mode: Mode,
    items: Vec<String>,
    used_names: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Mode {
    Args,
    Decode,
}

#[derive(Debug, Clone, Copy)]
enum StructRole {
    Args,
    Decode,
}

impl TypeGenerator {
    fn new(config: &SwiftTargetConfig, mode: Mode) -> Self {
        Self {
            config: config.clone(),
            mode,
            items: Vec::new(),
            used_names: BTreeSet::new(),
        }
    }

    pub(crate) fn finish(self) -> String {
        self.items.concat()
    }

    pub(crate) fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    fn render_fields(&mut self, fields: &[Field], owner_name: &str) -> Vec<RenderedField> {
        let mut used = BTreeSet::new();
        fields
            .iter()
            .map(|field| {
                let base_name = camel_case(&field.name);
                let swift_name = dedupe(&mut used, &base_name);
                let hint = format!("{owner_name}{}", pascal_case(&field.name));
                let (ty, wrapper) = self.render_property_type(&field.r#type, field.required, &hint);
                RenderedField {
                    swift_name,
                    original_name: field.name.clone(),
                    ty,
                    wrapper,
                }
            })
            .collect()
    }

    pub(crate) fn render_type(&mut self, node: &TypeNode, required: bool, hint: &str) -> String {
        let mut ty = match node {
            TypeNode::String => "String".into(),
            TypeNode::Float64 => "Double".into(),
            TypeNode::Int64 => "Int64".into(),
            TypeNode::Boolean => "Bool".into(),
            TypeNode::Null => "RustexNull".into(),
            TypeNode::Bytes => "Data".into(),
            TypeNode::Any | TypeNode::Unknown { .. } => "AnyCodable".into(),
            TypeNode::LiteralString { value } => {
                self.push_literal_enum_named(hint, std::slice::from_ref(value))
            }
            TypeNode::LiteralNumber { .. } => "Double".into(),
            TypeNode::LiteralBoolean { .. } => "Bool".into(),
            TypeNode::Id { table } => format!("{}Id", pascal_case(table)),
            TypeNode::Array { element } => {
                let inner = self.render_type(element, true, &format!("{hint}Item"));
                format!("[{inner}]")
            }
            TypeNode::Record { value } => {
                let inner = self.render_type(value, true, &format!("{hint}Value"));
                format!("[String: {inner}]")
            }
            TypeNode::Object { fields, .. } => {
                let name = self.claim_name(hint);
                let rendered = self.render_fields(fields, &name);
                let role = match self.mode {
                    Mode::Args => StructRole::Args,
                    Mode::Decode => StructRole::Decode,
                };
                self.push_struct_body(&name, &rendered, role);
                name
            }
            TypeNode::Union { members } => self.render_union(members, hint),
        };
        if !required {
            ty.push('?');
        }
        ty
    }

    fn render_property_type(
        &mut self,
        node: &TypeNode,
        required: bool,
        hint: &str,
    ) -> (String, Option<&'static str>) {
        match (self.mode, node, required) {
            (Mode::Decode, TypeNode::Float64, true) => ("Double".into(), Some("ConvexFloat")),
            (Mode::Decode, TypeNode::Float64, false) => {
                ("Double?".into(), Some("OptionalConvexFloat"))
            }
            (Mode::Decode, TypeNode::Int64, true) => ("Int64".into(), Some("ConvexInt")),
            (Mode::Decode, TypeNode::Int64, false) => ("Int64?".into(), Some("OptionalConvexInt")),
            _ => (self.render_type(node, required, hint), None),
        }
    }

    fn render_union(&mut self, members: &[TypeNode], hint: &str) -> String {
        if let Some(non_null) = optional_member(members) {
            let inner = self.render_type(non_null, true, hint);
            return format!("{inner}?");
        }
        if let Some(values) = literal_string_union(members) {
            return self.push_literal_enum_named(hint, &values);
        }
        if let Some((tag, variants)) = discriminated_union_members(members) {
            return self.push_discriminated_enum_named(hint, &tag, &variants);
        }
        "AnyCodable".into()
    }

    fn push_literal_enum_named(&mut self, name: &str, values: &[String]) -> String {
        let access = self.config.access_level.as_swift();
        let name = self.claim_name(name);
        self.items.push(format!(
            "{access} enum {name}: String, Codable, Equatable {{\n"
        ));
        let mut used = BTreeSet::new();
        for value in values {
            let case_name = dedupe(&mut used, &enum_case(value));
            self.items
                .push(format!("  case {case_name} = \"{}\"\n", escape(value)));
        }
        self.items.push(format!(
            "}}\n\nextension {name}: ConvexEncodable {{}}\nextension {name}: RustexConvexValueConvertible {{\n  public func rustexConvexValue() throws -> ConvexEncodable? {{ rawValue }}\n}}\n\n"
        ));
        name
    }

    fn push_discriminated_enum_named(
        &mut self,
        name: &str,
        tag: &str,
        variants: &[(String, Vec<Field>)],
    ) -> String {
        let access = self.config.access_level.as_swift();
        let name = self.claim_name(name);
        let coding_key = escape_keyword(&camel_case(tag));
        let mut variant_entries = Vec::new();
        for (value, fields) in variants {
            let payload_name = self.claim_name(&format!("{name}{}", pascal_case(value)));
            let rendered = self.render_fields(fields, &payload_name);
            self.push_struct_body(&payload_name, &rendered, StructRole::Decode);
            variant_entries.push((value.clone(), enum_case(value), payload_name));
        }

        self.items
            .push(format!("{access} enum {name}: Decodable, Equatable {{\n"));
        for (_, case_name, payload_name) in &variant_entries {
            self.items
                .push(format!("  case {case_name}({payload_name})\n"));
        }
        self.items
            .push("\n  enum CodingKeys: String, CodingKey {\n".into());
        self.items
            .push(format!("    case {coding_key} = \"{}\"\n", escape(tag)));
        self.items.push("  }\n\n".into());
        self.items
            .push("  public init(from decoder: Decoder) throws {\n".into());
        self.items
            .push("    let container = try decoder.container(keyedBy: CodingKeys.self)\n".into());
        self.items.push(format!(
            "    let tag = try container.decode(String.self, forKey: .{coding_key})\n"
        ));
        self.items.push("    switch tag {\n".into());
        for (value, case_name, payload_name) in &variant_entries {
            self.items.push(format!(
                "    case \"{}\":\n      self = .{case_name}(try {payload_name}(from: decoder))\n",
                escape(value)
            ));
        }
        self.items
            .push("    default:\n      throw DecodingError.dataCorruptedError(forKey: .".into());
        self.items.push(format!(
            "{coding_key}, in: container, debugDescription: \"Unknown {tag} value \\(tag)\")\n"
        ));
        self.items.push("    }\n  }\n}\n\n".into());
        name
    }

    fn push_struct_named(&mut self, name: &str, fields: &[RenderedField], role: StructRole) {
        let name = self.claim_name(name);
        self.push_struct_body(&name, fields, role);
    }

    fn push_struct_body(&mut self, name: &str, fields: &[RenderedField], role: StructRole) {
        let access = self.config.access_level.as_swift();
        let conformance = match role {
            StructRole::Args => "Codable, Equatable, RustexConvexArgs",
            StructRole::Decode => {
                if fields.iter().any(|field| field.wrapper.is_some()) {
                    "Decodable, Equatable"
                } else {
                    "Codable, Equatable"
                }
            }
        };
        self.items
            .push(format!("{access} struct {name}: {conformance} {{\n"));
        for field in fields {
            if let Some(wrapper) = field.wrapper {
                self.items.push(format!("  @{wrapper}\n"));
            }
            let keyword = if field.wrapper.is_some() {
                "var"
            } else {
                "let"
            };
            self.items.push(format!(
                "  {access} {keyword} {}: {}\n",
                field.swift_name, field.ty
            ));
        }
        self.push_coding_keys(fields);
        self.push_initializer(fields, role);
        if matches!(role, StructRole::Args) {
            self.push_convex_args(fields);
        }
        self.items.push("}\n\n".into());
        if matches!(role, StructRole::Args) {
            self.items.push(format!(
                "extension {name}: ConvexEncodable {{}}\nextension {name}: RustexConvexValueConvertible {{\n  public func rustexConvexValue() throws -> ConvexEncodable? {{ self }}\n}}\n\n"
            ));
        }
    }

    fn push_coding_keys(&mut self, fields: &[RenderedField]) {
        self.items
            .push("\n  enum CodingKeys: String, CodingKey {\n".into());
        for field in fields {
            if field.swift_name == field.original_name {
                self.items.push(format!("    case {}\n", field.swift_name));
            } else {
                self.items.push(format!(
                    "    case {} = \"{}\"\n",
                    field.swift_name,
                    escape(&field.original_name)
                ));
            }
        }
        self.items.push("  }\n".into());
    }

    fn push_initializer(&mut self, fields: &[RenderedField], role: StructRole) {
        let access = self.config.access_level.as_swift();
        let params = fields
            .iter()
            .map(|field| format!("{}: {}", field.swift_name, field.ty))
            .collect::<Vec<_>>()
            .join(", ");
        self.items.push(format!("\n  {access} init({params}) {{\n"));
        for field in fields {
            self.items.push(format!(
                "    self.{} = {}\n",
                field.swift_name, field.swift_name
            ));
        }
        self.items.push("  }\n".into());
        if matches!(role, StructRole::Decode) && fields.iter().any(|field| field.wrapper.is_some())
        {
            self.items
                .push("\n  public init(from decoder: Decoder) throws {\n".into());
            self.items.push(
                "    let container = try decoder.container(keyedBy: CodingKeys.self)\n".into(),
            );
            for field in fields {
                let decode_ty = wrapper_decode_type(field).unwrap_or(field.ty.as_str());
                self.items.push(format!(
                    "    self.{} = try container.decode({decode_ty}.self, forKey: .{}){}\n",
                    field.swift_name,
                    field.swift_name,
                    if field.wrapper.is_some() {
                        ".wrappedValue"
                    } else {
                        ""
                    }
                ));
            }
            self.items.push("  }\n".into());
        }
    }

    fn push_convex_args(&mut self, fields: &[RenderedField]) {
        self.items
            .push("\n  public func convexArgs() throws -> [String: ConvexEncodable?] {\n".into());
        self.items.push("    [\n".into());
        for field in fields {
            self.items.push(format!(
                "      \"{}\": {},\n",
                escape(&field.original_name),
                format!("try {}.rustexConvexValue()", field.swift_name)
            ));
        }
        self.items.push("    ]\n  }\n".into());
    }

    fn claim_name(&mut self, base: &str) -> String {
        dedupe(&mut self.used_names, &pascal_case(base))
    }
}

fn render_table(table: &Table, generator: &mut TypeGenerator) {
    if let TypeNode::Object { fields, .. } = &table.document_type {
        let mut table_fields = vec![
            Field {
                name: "_id".into(),
                required: true,
                r#type: TypeNode::Id {
                    table: table.name.clone(),
                },
                doc: None,
                source: None,
            },
            Field {
                name: "_creationTime".into(),
                required: true,
                r#type: TypeNode::Float64,
                doc: None,
                source: None,
            },
        ];
        table_fields.extend(fields.clone());
        let rendered = generator.render_fields(&table_fields, &table.doc_name);
        generator.push_struct_named(&table.doc_name, &rendered, StructRole::Decode);
    }
}

fn prelude(config: &SwiftTargetConfig) -> String {
    format!(
        "import ConvexMobile\nimport Foundation\n@_exported import {}\n\n",
        config.runtime_module_name
    )
}

fn optional_member(members: &[TypeNode]) -> Option<&TypeNode> {
    if members.len() == 2
        && members
            .iter()
            .any(|member| matches!(member, TypeNode::Null))
    {
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
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn discriminated_union_members(
    members: &[TypeNode],
) -> Option<(String, Vec<(String, Vec<Field>)>)> {
    let objects = members
        .iter()
        .map(|member| match member {
            TypeNode::Object { fields, .. } => Some(fields.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let candidate_tags = objects
        .first()?
        .iter()
        .filter_map(|field| match &field.r#type {
            TypeNode::LiteralString { .. } => Some(field.name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for tag in candidate_tags {
        let mut variants = Vec::new();
        let mut seen = BTreeSet::new();
        let mut valid = true;
        for fields in &objects {
            let Some(discriminant) = fields.iter().find(|field| field.name == tag) else {
                valid = false;
                break;
            };
            let TypeNode::LiteralString { value } = &discriminant.r#type else {
                valid = false;
                break;
            };
            if !seen.insert(value.clone()) {
                valid = false;
                break;
            }
            variants.push((
                value.clone(),
                fields
                    .iter()
                    .filter(|field| field.name != tag)
                    .cloned()
                    .collect(),
            ));
        }
        if valid {
            return Some((tag, variants));
        }
    }
    None
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn wrapper_decode_type(field: &RenderedField) -> Option<&'static str> {
    match field.wrapper {
        Some("ConvexFloat") => Some("ConvexFloat<Double>"),
        Some("OptionalConvexFloat") => Some("OptionalConvexFloat<Double>"),
        Some("ConvexInt") => Some("ConvexInt<Int64>"),
        Some("OptionalConvexInt") => Some("OptionalConvexInt<Int64>"),
        _ => None,
    }
}
