use std::collections::BTreeSet;

pub fn pascal_case(input: &str) -> String {
    let mut out = String::new();
    for part in input.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        "Value".into()
    } else if out
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("V{out}")
    } else {
        out
    }
}

pub fn camel_case(input: &str) -> String {
    let pascal = pascal_case(input);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(first) => {
            let mut out = first.to_lowercase().collect::<String>();
            out.push_str(chars.as_str());
            escape_keyword(&out)
        }
        None => "value".into(),
    }
}

pub fn enum_case(input: &str) -> String {
    camel_case(input)
}

pub fn module_name(module_path: &str) -> String {
    let joined = module_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(pascal_case)
        .collect::<Vec<_>>()
        .join("");
    if joined.is_empty() {
        "Root".into()
    } else {
        joined
    }
}

pub fn dedupe(used: &mut BTreeSet<String>, base: &str) -> String {
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

pub fn escape_keyword(input: &str) -> String {
    if SWIFT_KEYWORDS.contains(&input) {
        format!("{input}_")
    } else {
        input.into()
    }
}

const SWIFT_KEYWORDS: &[&str] = &[
    "Any",
    "Self",
    "as",
    "associatedtype",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "defer",
    "deinit",
    "do",
    "else",
    "enum",
    "extension",
    "fallthrough",
    "false",
    "fileprivate",
    "for",
    "func",
    "guard",
    "if",
    "import",
    "in",
    "init",
    "inout",
    "internal",
    "is",
    "let",
    "nil",
    "open",
    "operator",
    "private",
    "protocol",
    "public",
    "repeat",
    "return",
    "rethrows",
    "self",
    "static",
    "struct",
    "subscript",
    "super",
    "switch",
    "throw",
    "throws",
    "true",
    "try",
    "typealias",
    "var",
    "where",
    "while",
];
