use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSpan {
    pub file: Utf8PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub symbol: Option<String>,
    pub provenance: Option<String>,
    pub suggestion: Option<String>,
    pub primary_span: Option<SourceSpan>,
    #[serde(default)]
    pub related_spans: Vec<SourceSpan>,
    pub snippet: Option<String>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            symbol: None,
            provenance: None,
            suggestion: None,
            primary_span: None,
            related_spans: Vec::new(),
            snippet: None,
        }
    }
}
