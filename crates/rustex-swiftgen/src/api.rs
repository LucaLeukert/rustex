use crate::naming::{module_name, pascal_case};
use crate::types::{Mode, api_type_generator, render_args_type, render_output_type};
use rustex_ir::{Function, FunctionKind, IrPackage};
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
        }
        specs.push_str("  }\n\n");
    }
    specs.push_str("}\n");

    format!(
        "import ConvexMobile\nimport Foundation\nimport {}\n\n{}{}",
        config.runtime_module_name,
        generator.finish(),
        specs
    )
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
