use rustex_ir::{Function, IrPackage, Table};
use sha2::{Digest, Sha256};

pub fn finalize_ir(mut package: IrPackage) -> IrPackage {
    package.tables.sort_by(|a, b| a.name.cmp(&b.name));
    package
        .functions
        .sort_by(|a, b| a.canonical_path.cmp(&b.canonical_path));
    package.manifest_meta.input_hash = compute_hash(&package.tables, &package.functions);
    package
}

fn compute_hash(tables: &[Table], functions: &[Function]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(tables).expect("tables serialize"));
    hasher.update(serde_json::to_vec(functions).expect("functions serialize"));
    format!("{:x}", hasher.finalize())
}
