//! Human and grep-friendly renderings of `issue view`.
use crate::error::{AppError, Result};
use serde_json::{Map, Value};

/// Top-level keys of the `issue view` output, taken from its JSON schema.
pub fn issue_view_fields() -> Vec<String> {
    let schema = schemars::schema_for!(crate::model::IssueView);
    let mut keys: Vec<String> = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    keys
}

pub fn validate_fields(fields: &[String]) -> Result<()> {
    let valid = issue_view_fields();
    let unknown: Vec<&str> = fields
        .iter()
        .map(|f| f.split('.').next().unwrap_or_default())
        .filter(|top| !valid.iter().any(|v| v == top))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let mut e = AppError::input(format!(
        "Unknown field {}. Valid fields: {}",
        unknown.join(", "),
        valid.join(", ")
    ));
    e.details = Box::new(serde_json::json!({"unknown":unknown,"valid":valid}));
    Err(e)
}

/// Follow a dotted path; arrays map the rest of the path over their items.
fn lookup(value: &Value, path: &[&str]) -> Value {
    let Some((head, rest)) = path.split_first() else {
        return value.clone();
    };
    match value {
        Value::Array(items) => Value::Array(items.iter().map(|i| lookup(i, path)).collect()),
        Value::Object(map) => map.get(*head).map_or(Value::Null, |v| lookup(v, rest)),
        _ => Value::Null,
    }
}

/// Drop control characters (ANSI/OSC escapes) from API text, keeping newlines and tabs.
pub fn printable(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

fn plain(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => printable(s),
        Value::Array(items) => items
            .iter()
            .map(plain)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(_) => value.to_string(),
        other => other.to_string(),
    }
}

pub fn select(data: &Value, fields: &[String]) -> Map<String, Value> {
    fields
        .iter()
        .map(|f| (f.clone(), lookup(data, &f.split('.').collect::<Vec<_>>())))
        .collect()
}

pub fn fields_text(data: &Value, fields: &[String]) -> String {
    let selected = select(data, fields);
    if let [only] = fields {
        return plain(&selected[only]);
    }
    fields
        .iter()
        .map(|f| format!("{f}: {}", plain(&selected[f])))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn issue_text(data: &Value) -> String {
    let get = |path: &str| plain(&lookup(data, &path.split('.').collect::<Vec<_>>()));
    let or_none = |s: String| if s.is_empty() { "-".to_owned() } else { s };
    let heading = [get("identifier"), get("title")]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("  ");
    let mut out = vec![
        heading,
        format!("State:    {}", or_none(get("state.name"))),
        format!("Assignee: {}", or_none(get("assignee.name"))),
        format!("Labels:   {}", or_none(get("labels.name"))),
    ];
    let url = get("url");
    if !url.is_empty() {
        out.push(format!("URL:      {url}"));
    }
    let description = get("description");
    out.push(String::new());
    out.push(if description.trim().is_empty() {
        "(no description)".into()
    } else {
        description.trim_end().to_owned()
    });
    let mut relations = vec![];
    for (group, label, other) in [
        ("blocks", "blocks", "related_issue"),
        ("blocked_by", "blocked by", "issue"),
        ("related", "related", ""),
        ("duplicate_of", "duplicate of", "related_issue"),
        ("duplicated_by", "duplicated by", "issue"),
        ("other", "", ""),
    ] {
        for r in data["relations"][group].as_array().into_iter().flatten() {
            let other = match other {
                "" if r["related_issue"]["id"] == data["id"] => "issue",
                "" => "related_issue",
                side => side,
            };
            let label = if label.is_empty() {
                plain(&r["type"])
            } else {
                label.to_owned()
            };
            let issue = [plain(&r[other]["identifier"]), plain(&r[other]["title"])]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("  ");
            relations.push(format!("  {label:<13} {issue}"));
        }
    }
    if !relations.is_empty() {
        out.push(String::new());
        out.push(format!("Relations ({})", relations.len()));
        out.extend(relations);
    }
    if let Some(comments) = data["comments"].as_array() {
        out.push(String::new());
        out.push(format!("Comments ({})", comments.len()));
        for c in comments {
            let author = or_none(plain(&c["user"]["name"]));
            out.push(format!("\n— {author}, {}", plain(&c["created_at"])));
            out.push(plain(&c["body"]).trim_end().to_owned());
        }
    }
    out.join("\n")
}
