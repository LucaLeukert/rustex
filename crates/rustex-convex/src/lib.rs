use rustex_ir::{
    CapabilityFlags, Constraint, ConstraintKind, ContractProvenance, Function, FunctionKind,
    IrPackage, NamedType, Origin, SourceInventoryItem, SourceKind, Table, TypeNode, Visibility,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use tracing::debug;

pub fn finalize_ir(mut package: IrPackage) -> IrPackage {
    let _span = tracing::info_span!(
        "rustex_convex.finalize_ir",
        tables = package.tables.len(),
        functions = package.functions.len()
    )
    .entered();
    package.tables.sort_by(|a, b| a.name.cmp(&b.name));
    package
        .functions
        .sort_by(|a, b| a.canonical_path.cmp(&b.canonical_path));
    package.named_types = collect_named_types(&package.tables, &package.functions);
    package.constraints = collect_constraints(&package.tables, &package.functions);
    package.capabilities = collect_capabilities(
        &package.functions,
        &package.project,
        &package.source_inventory,
    );
    package.source_inventory = normalize_source_inventory(&package.source_inventory);
    package.manifest_meta.input_hash = compute_hash(&package);
    debug!(
        named_types = package.named_types.len(),
        constraints = package.constraints.len(),
        "finalized IR package"
    );
    package
}

fn compute_hash(package: &IrPackage) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(package).expect("package serializes"));
    format!("{:x}", hasher.finalize())
}

fn collect_named_types(tables: &[Table], functions: &[Function]) -> Vec<NamedType> {
    let mut named_types = Vec::new();
    let mut seen = BTreeSet::new();

    for table in tables {
        walk_named_type(
            &table.document_type,
            &format!("table.{}.document", table.name),
            &table.doc_name,
            table.source.as_ref(),
            &mut seen,
            &mut named_types,
        );
    }

    for function in functions {
        if let Some(args) = &function.args_type {
            walk_named_type(
                args,
                &format!("function.{}.args", function.canonical_path),
                &format!("{}Args", pascal_case(&function.export_name)),
                function.source.as_ref(),
                &mut seen,
                &mut named_types,
            );
        }
        if let Some(returns) = &function.returns_type {
            walk_named_type(
                returns,
                &format!("function.{}.returns", function.canonical_path),
                &format!("{}Response", pascal_case(&function.export_name)),
                function.source.as_ref(),
                &mut seen,
                &mut named_types,
            );
        }
    }

    named_types.sort_by(|a, b| a.key.cmp(&b.key));
    named_types
}

fn walk_named_type(
    node: &TypeNode,
    key: &str,
    suggested_name: &str,
    source: Option<&Origin>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<NamedType>,
) {
    if matches!(node, TypeNode::Object { .. } | TypeNode::Union { .. }) && seen.insert(key.into()) {
        out.push(NamedType {
            key: key.into(),
            suggested_name: suggested_name.into(),
            origin_symbol: key.into(),
            node: node.clone(),
            source: source.cloned(),
        });
    }

    match node {
        TypeNode::Array { element } => {
            walk_named_type(
                element,
                &format!("{key}.item"),
                &format!("{suggested_name}Item"),
                source,
                seen,
                out,
            );
        }
        TypeNode::Record { value } => {
            walk_named_type(
                value,
                &format!("{key}.value"),
                &format!("{suggested_name}Value"),
                source,
                seen,
                out,
            );
        }
        TypeNode::Object { fields, .. } => {
            for field in fields {
                walk_named_type(
                    &field.r#type,
                    &format!("{key}.{}", field.name),
                    &format!("{suggested_name}{}", pascal_case(&field.name)),
                    field.source.as_ref().or(source),
                    seen,
                    out,
                );
            }
        }
        TypeNode::Union { members } => {
            for (index, member) in members.iter().enumerate() {
                walk_named_type(
                    member,
                    &format!("{key}.variant{index}"),
                    &format!("{suggested_name}Variant{}", index + 1),
                    source,
                    seen,
                    out,
                );
            }
        }
        _ => {}
    }
}

fn collect_constraints(tables: &[Table], functions: &[Function]) -> Vec<Constraint> {
    let mut constraints = Vec::new();
    for table in tables {
        collect_node_constraints(
            &table.document_type,
            &format!("table.{}.document", table.name),
            &mut constraints,
        );
    }
    for function in functions {
        if let Some(args) = &function.args_type {
            collect_node_constraints(
                args,
                &format!("function.{}.args", function.canonical_path),
                &mut constraints,
            );
        }
        if let Some(returns) = &function.returns_type {
            collect_node_constraints(
                returns,
                &format!("function.{}.returns", function.canonical_path),
                &mut constraints,
            );
        }
    }
    constraints
}

fn collect_node_constraints(node: &TypeNode, path: &str, constraints: &mut Vec<Constraint>) {
    match node {
        TypeNode::LiteralString { value } => constraints.push(Constraint {
            path: path.into(),
            kind: ConstraintKind::Literal,
            detail: value.clone(),
        }),
        TypeNode::LiteralNumber { value } => constraints.push(Constraint {
            path: path.into(),
            kind: ConstraintKind::Literal,
            detail: value.to_string(),
        }),
        TypeNode::LiteralBoolean { value } => constraints.push(Constraint {
            path: path.into(),
            kind: ConstraintKind::Literal,
            detail: value.to_string(),
        }),
        TypeNode::Id { table } => constraints.push(Constraint {
            path: path.into(),
            kind: ConstraintKind::IdentifierTable,
            detail: table.clone(),
        }),
        TypeNode::Record { value } => {
            constraints.push(Constraint {
                path: path.into(),
                kind: ConstraintKind::RecordValue,
                detail: "string_keyed_record".into(),
            });
            collect_node_constraints(value, &format!("{path}.value"), constraints);
        }
        TypeNode::Array { element } => {
            collect_node_constraints(element, &format!("{path}.item"), constraints);
        }
        TypeNode::Object { fields, .. } => {
            for field in fields {
                if !field.required {
                    constraints.push(Constraint {
                        path: format!("{path}.{}", field.name),
                        kind: ConstraintKind::Optional,
                        detail: "nullable_or_optional".into(),
                    });
                }
                collect_node_constraints(
                    &field.r#type,
                    &format!("{path}.{}", field.name),
                    constraints,
                );
            }
        }
        TypeNode::Union { members } => {
            if let Some(tag) = discriminant_tag(members) {
                constraints.push(Constraint {
                    path: path.into(),
                    kind: ConstraintKind::Discriminant,
                    detail: tag,
                });
            }
            for (index, member) in members.iter().enumerate() {
                collect_node_constraints(member, &format!("{path}.variant{index}"), constraints);
            }
        }
        _ => {}
    }
}

fn discriminant_tag(members: &[TypeNode]) -> Option<String> {
    let mut candidate = None;
    for member in members {
        let TypeNode::Object { fields, .. } = member else {
            return None;
        };
        let this_candidate = fields.iter().find_map(|field| match field.r#type {
            TypeNode::LiteralString { .. } => Some(field.name.clone()),
            _ => None,
        })?;
        if let Some(existing) = &candidate {
            if existing != &this_candidate {
                return None;
            }
        } else {
            candidate = Some(this_candidate);
        }
    }
    candidate
}

fn collect_capabilities(
    functions: &[Function],
    project: &rustex_ir::ProjectInfo,
    source_inventory: &[SourceInventoryItem],
) -> CapabilityFlags {
    CapabilityFlags {
        generated_metadata_present: project.generated_metadata_present,
        inferred_returns_used: functions
            .iter()
            .any(|function| matches!(function.contract_provenance, ContractProvenance::Inferred)),
        internal_functions_present: functions
            .iter()
            .any(|function| matches!(function.visibility, Visibility::Internal)),
        public_functions_present: functions
            .iter()
            .any(|function| matches!(function.visibility, Visibility::Public)),
        http_actions_present: functions.iter().any(|function| {
            matches!(function.kind, FunctionKind::Action) && function.module_path.ends_with("http")
        }),
        components_present: functions
            .iter()
            .any(|function| function.component_path.is_some())
            || source_inventory
                .iter()
                .any(|item| matches!(item.kind, SourceKind::ComponentModule)),
    }
}

fn normalize_source_inventory(
    source_inventory: &[SourceInventoryItem],
) -> Vec<SourceInventoryItem> {
    let mut items = source_inventory.to_vec();
    items.sort_by(|a, b| a.path.cmp(&b.path));
    items.dedup_by(|a, b| a.path == b.path && a.kind == b.kind);
    items
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
