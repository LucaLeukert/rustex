use camino::Utf8PathBuf;
use rustex_diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IrPackage {
    pub project: ProjectInfo,
    #[serde(default)]
    pub tables: Vec<Table>,
    #[serde(default)]
    pub functions: Vec<Function>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub manifest_meta: ManifestMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectInfo {
    pub name: String,
    pub root: Utf8PathBuf,
    pub convex_root: Utf8PathBuf,
    pub convex_version: Option<String>,
    pub generated_metadata_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestMeta {
    pub rustex_version: String,
    pub manifest_version: u32,
    pub input_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Table {
    pub name: String,
    pub doc_name: String,
    pub document_type: TypeNode,
    pub source: Option<Origin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub required: bool,
    pub r#type: TypeNode,
    pub doc: Option<String>,
    pub source: Option<Origin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Function {
    pub canonical_path: String,
    pub module_path: String,
    pub export_name: String,
    pub visibility: Visibility,
    pub kind: FunctionKind,
    pub args_type: Option<TypeNode>,
    pub returns_type: Option<TypeNode>,
    pub contract_provenance: ContractProvenance,
    pub source: Option<Origin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FunctionKind {
    Query,
    Mutation,
    Action,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContractProvenance {
    Validator,
    GeneratedTs,
    Inferred,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Origin {
    pub file: Utf8PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeNode {
    String,
    Float64,
    Int64,
    Boolean,
    Null,
    Bytes,
    Any,
    LiteralString { value: String },
    LiteralNumber { value: f64 },
    LiteralBoolean { value: bool },
    Id { table: String },
    Array { element: Box<TypeNode> },
    Record { value: Box<TypeNode> },
    Object { fields: Vec<Field>, open: bool },
    Union { members: Vec<TypeNode> },
    Unknown { reason: String, confidence: f32 },
}

impl TypeNode {
    pub fn object(fields: Vec<Field>) -> Self {
        Self::Object {
            fields,
            open: false,
        }
    }
}
