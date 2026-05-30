use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use serde::Deserialize;

use crate::errors::{BridgeError, BridgeResult};

const MAX_INTAKE_LINES: usize = 4096;
const MAX_INTAKE_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IfcIntake {
    pub description: Option<String>,
    pub project_guid: Option<String>,
    pub project_name: Option<String>,
    pub author: Option<String>,
    pub originating_system: Option<String>,
    pub approximate_entity_count: usize,
}

#[derive(Debug, Clone)]
pub struct IfcPreviewElement {
    pub step_id: String,
    pub type_name: String,
    pub name: Option<String>,
}

impl IfcIntake {
    pub fn routing_key(&self) -> Option<String> {
        if let Some(guid) = &self.project_guid {
            return Some(guid.clone());
        }
        self.structural_fingerprint()
    }

    pub fn structural_fingerprint(&self) -> Option<String> {
        let seed = format!(
            "{}|{}|{}",
            self.project_name.as_deref().unwrap_or_default(),
            self.author.as_deref().unwrap_or_default(),
            self.approximate_entity_count
        );
        if seed.trim_matches('|').is_empty() {
            return None;
        }
        Some(format!(
            "fingerprint:{}",
            blake3::hash(seed.as_bytes()).to_hex()
        ))
    }
}

pub fn hash_file(path: &Path) -> BridgeResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn parse_intake(path: &Path) -> BridgeResult<IfcIntake> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut text = String::new();
    let mut bytes = 0usize;
    let mut line = String::new();

    for _ in 0..MAX_INTAKE_LINES {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        bytes += n;
        if bytes > MAX_INTAKE_BYTES {
            break;
        }
        text.push_str(&line);
        let upper = text.to_ascii_uppercase();
        if upper.contains("IFCPROJECT") && upper.contains("ENDSEC;") {
            break;
        }
    }

    if text.trim().is_empty() {
        return Err(BridgeError::Config(format!(
            "{} is empty or unreadable as IFC",
            path.display()
        )));
    }

    let mut out = IfcIntake {
        approximate_entity_count: count_entity_starts(&text),
        ..IfcIntake::default()
    };

    if let Some(args) = find_call_args(&text, "FILE_DESCRIPTION") {
        let parts = split_top_level_args(args);
        if let Some(first) = parts.first() {
            let descriptions = parse_string_list(first);
            out.description = descriptions.into_iter().next();
        }
    }

    if let Some(args) = find_call_args(&text, "FILE_NAME") {
        let parts = split_top_level_args(args);
        if let Some(author_arg) = parts.get(2) {
            out.author = parse_string_list(author_arg).into_iter().next();
        }
        if let Some(system_arg) = parts.get(5) {
            out.originating_system = parse_step_string(system_arg.trim());
        }
    }

    if let Some(args) = find_call_args(&text, "IFCPROJECT") {
        let parts = split_top_level_args(args);
        if let Some(guid_arg) = parts.first() {
            out.project_guid = parse_step_string(guid_arg.trim());
        }
        if let Some(name_arg) = parts.get(2) {
            out.project_name = parse_step_string(name_arg.trim());
        }
    }

    Ok(out)
}

pub fn parse_preview_elements(path: &Path, limit: usize) -> BridgeResult<Vec<IfcPreviewElement>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }
        let Some((step_id, rest)) = trimmed.split_once('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(open_paren) = rest.find('(') else {
            continue;
        };
        let raw_type = rest[..open_paren].trim();
        if !raw_type.to_ascii_uppercase().starts_with("IFC") || !is_preview_entity(raw_type) {
            continue;
        }
        let args = balanced_parens(&rest[open_paren..]).unwrap_or_default();
        let parts = split_top_level_args(args);
        let name = parts.get(2).and_then(|value| parse_step_string(value));
        out.push(IfcPreviewElement {
            step_id: step_id.trim().to_string(),
            type_name: raw_type.to_string(),
            name,
        });
        if out.len() >= limit {
            break;
        }
    }

    Ok(out)
}

fn is_preview_entity(type_name: &str) -> bool {
    let upper = type_name.to_ascii_uppercase();
    if !upper.starts_with("IFC") {
        return false;
    }
    const SKIP_PREFIXES: &[&str] = &[
        "IFCREL",
        "IFCPROPERTY",
        "IFCPROPERTYS",
        "IFCQUANTITY",
        "IFCOWNER",
        "IFCPERSON",
        "IFCORGANIZATION",
        "IFCAPPLICATION",
        "IFCSIUNIT",
        "IFCUNIT",
        "IFCMEASURE",
        "IFCCARTESIAN",
        "IFCDIRECTION",
        "IFCAXIS",
        "IFCLOCALPLACEMENT",
        "IFCGEOMETRIC",
        "IFCSHAPE",
        "IFCPRODUCTDEFINITIONSHAPE",
        "IFCSTYLE",
        "IFCCOLOUR",
        "IFCMATERIAL",
        "IFCPRESENTATION",
        "IFCTOPOLOGY",
        "IFCCONNECTION",
    ];
    !SKIP_PREFIXES.iter().any(|prefix| upper.starts_with(prefix))
}

fn count_entity_starts(text: &str) -> usize {
    text.lines()
        .filter(|line| line.trim_start().starts_with('#'))
        .count()
}

fn find_call_args<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let upper = text.to_ascii_uppercase();
    let needle = name.to_ascii_uppercase();
    let mut search_from = 0usize;
    while let Some(rel) = upper[search_from..].find(&needle) {
        let start = search_from + rel;
        let before_ok = start == 0
            || !upper.as_bytes()[start - 1].is_ascii_alphanumeric()
                && upper.as_bytes()[start - 1] != b'_';
        let mut cursor = start + needle.len();
        while matches!(
            upper.as_bytes().get(cursor),
            Some(b' ' | b'\t' | b'\r' | b'\n')
        ) {
            cursor += 1;
        }
        if before_ok && matches!(upper.as_bytes().get(cursor), Some(b'(')) {
            return balanced_parens(&text[cursor..]);
        }
        search_from = start + needle.len();
    }
    None
}

fn balanced_parens(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.first().copied() != Some(b'(') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                if in_string && matches!(bytes.get(i + 1), Some(b'\'')) {
                    i += 2;
                    continue;
                }
                in_string = !in_string;
            }
            b'(' if !in_string => depth += 1,
            b')' if !in_string => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return text.get(1..i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn split_top_level_args(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                if in_string && matches!(bytes.get(i + 1), Some(b'\'')) {
                    i += 2;
                    continue;
                }
                in_string = !in_string;
            }
            b'(' if !in_string => depth += 1,
            b')' if !in_string => depth = depth.saturating_sub(1),
            b',' if !in_string && depth == 0 => {
                out.push(text[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start <= text.len() {
        out.push(text[start..].trim());
    }
    out
}

fn parse_string_list(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if let Some((value, next)) = parse_step_string_at(text, i) {
                out.push(value);
                i = next;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn parse_step_string(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.starts_with('\'') {
        return None;
    }
    parse_step_string_at(trimmed, 0).map(|(value, _)| value)
}

fn parse_step_string_at(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start).copied() != Some(b'\'') {
        return None;
    }
    let mut out = String::new();
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' if matches!(bytes.get(i + 1), Some(b'\'')) => {
                out.push('\'');
                i += 2;
            }
            b'\'' => return Some((out, i + 1)),
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header_and_project() {
        let text = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');\nFILE_NAME('tower.ifc','2026-05-24T12:34:00',('Lawrence'),('Planmorph'),'x','Revit 2024','');\nENDSEC;\nDATA;\n#1=IFCPROJECT('2HnQxDrSH5sBbC4NkVOGR8',$,'Commercial Tower',$,$,$,$,(#2),#3);\n";
        let file_name = find_call_args(text, "FILE_NAME").expect("FILE_NAME");
        let file_parts = split_top_level_args(file_name);
        assert_eq!(
            parse_string_list(file_parts[2]).first().unwrap(),
            "Lawrence"
        );
        assert_eq!(parse_step_string(file_parts[5]).unwrap(), "Revit 2024");

        let project = find_call_args(text, "IFCPROJECT").expect("IFCPROJECT");
        let project_parts = split_top_level_args(project);
        assert_eq!(
            parse_step_string(project_parts[0]).unwrap(),
            "2HnQxDrSH5sBbC4NkVOGR8"
        );
        assert_eq!(
            parse_step_string(project_parts[2]).unwrap(),
            "Commercial Tower"
        );
    }

    #[test]
    fn handles_step_escaped_quotes() {
        assert_eq!(parse_step_string("'Bob''s Tower'").unwrap(), "Bob's Tower");
    }
}
