use crate::naming::{camel_case, module_name, pascal_case};
use crate::types::{Mode, api_type_generator, render_args_type, render_output_type};
use rustex_ir::{Field, Function, FunctionKind, IrPackage, TypeNode};
use rustex_project::SwiftTargetConfig;
use std::collections::BTreeMap;

pub fn api_swift(package: &IrPackage, config: &SwiftTargetConfig) -> String {
    let access = config.access_level.as_swift();
    let mut generator = api_type_generator(config);
    let mut specs = String::new();
    let mut grouped: BTreeMap<String, Vec<&Function>> = BTreeMap::new();
    for function in &package.functions {
        grouped
            .entry(function.module_path.clone())
            .or_default()
            .push(function);
    }

    specs.push_str(&format!("{access} enum API {{\n"));
    for (module_path, functions) in grouped {
        specs.push_str(&format!(
            "  {access} enum {} {{\n",
            module_name(&module_path)
        ));
        for function in functions {
            let base = pascal_case(&function.export_name);
            let args_name = format!("{base}Args");
            let output_name = format!("{base}Response");
            generator.set_mode(Mode::Args);
            let args_ty = render_args_type(function.args_type.as_ref(), &args_name, &mut generator);
            generator.set_mode(Mode::Decode);
            let output_ty =
                render_output_type(function.returns_type.as_ref(), &output_name, &mut generator);
            let protocol = match function.kind {
                FunctionKind::Query => "RustexQuerySpec",
                FunctionKind::Mutation => "RustexMutationSpec",
                FunctionKind::Action => "RustexActionSpec",
            };
            specs.push_str(&format!(
                "    {access} enum {base}: {protocol} {{\n      {access} typealias Args = {args_ty}\n      {access} typealias Output = {output_ty}\n      {access} static let path = \"{}\"\n    }}\n\n",
                escape(&function.canonical_path)
            ));
            specs.push_str(&operation_helper(function, &base, &args_name, access));
        }
        specs.push_str("  }\n\n");
    }
    specs.push_str("}\n");

    format!(
        "import ConvexMobile\nimport Foundation\n@_exported import {}\n\n{}{}",
        config.runtime_module_name,
        generator.finish(),
        specs
    )
}

fn operation_helper(function: &Function, base: &str, args_name: &str, access: &str) -> String {
    let helper_name = camel_case(&function.export_name);
    let call_ty = match function.kind {
        FunctionKind::Query => "RustexQueryCall",
        FunctionKind::Mutation => "RustexMutationCall",
        FunctionKind::Action => "RustexActionCall",
    };
    match &function.args_type {
        Some(TypeNode::Object { fields, .. }) if !fields.is_empty() => {
            let params = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        camel_case(&field.name),
                        helper_param_type(field, args_name)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let assignments = fields
                .iter()
                .map(|field| {
                    let name = camel_case(&field.name);
                    format!("{name}: {name}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "    {access} static func {helper_name}({params}) -> {call_ty}<{base}> {{\n      {call_ty}(args: {args_name}({assignments}))\n    }}\n\n"
            )
        }
        Some(TypeNode::Object { .. }) | None => format!(
            "    {access} static func {helper_name}() -> {call_ty}<{base}> {{\n      {call_ty}(args: RustexNoArgs())\n    }}\n\n"
        ),
        Some(_) => format!(
            "    {access} static func {helper_name}(_ args: {args_name}) -> {call_ty}<{base}> {{\n      {call_ty}(args: args)\n    }}\n\n"
        ),
    }
}

fn helper_param_type(field: &Field, owner_name: &str) -> String {
    helper_type(
        &field.r#type,
        field.required,
        &format!("{owner_name}{}", pascal_case(&field.name)),
    )
}

fn helper_type(node: &TypeNode, required: bool, hint: &str) -> String {
    let mut ty = match node {
        TypeNode::String => "String".into(),
        TypeNode::Float64 => "Double".into(),
        TypeNode::Int64 => "Int64".into(),
        TypeNode::Boolean => "Bool".into(),
        TypeNode::Null => "RustexNull".into(),
        TypeNode::Bytes => "Data".into(),
        TypeNode::Any | TypeNode::Unknown { .. } => "AnyCodable".into(),
        TypeNode::LiteralString { .. } => pascal_case(hint),
        TypeNode::LiteralNumber { .. } => "Double".into(),
        TypeNode::LiteralBoolean { .. } => "Bool".into(),
        TypeNode::Id { table } => format!("{}Id", pascal_case(table)),
        TypeNode::Array { element } => {
            let inner = helper_type(element, true, &format!("{hint}Item"));
            format!("[{inner}]")
        }
        TypeNode::Record { value } => {
            let inner = helper_type(value, true, &format!("{hint}Value"));
            format!("[String: {inner}]")
        }
        TypeNode::Object { .. } => pascal_case(hint),
        TypeNode::Union { members } => {
            if let Some(non_null) = optional_member(members) {
                return helper_type(non_null, false, hint);
            }
            if members
                .iter()
                .all(|member| matches!(member, TypeNode::LiteralString { .. }))
            {
                pascal_case(hint)
            } else {
                "AnyCodable".into()
            }
        }
    };
    if !required {
        ty.push('?');
    }
    ty
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

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
