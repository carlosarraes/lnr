use graphql_parser::schema::{Definition, TypeDefinition, Value, parse_schema};
use std::{env, fs, path::Path};

const SCHEMA: &str = "graphql/schema.graphql";

// Linear rejects deprecated root fields with a bare "deprecated" message, so the
// replacement hint has to come from the bundled schema. Linear marks some fields with
// @deprecated and others only with a "[DEPRECATED]" description prefix.
fn main() {
    println!("cargo:rerun-if-changed={SCHEMA}");
    let source = fs::read_to_string(SCHEMA).expect("read GraphQL schema");
    let schema = parse_schema::<String>(&source).expect("parse GraphQL schema");
    let mut entries = vec![];
    for definition in &schema.definitions {
        let Definition::TypeDefinition(TypeDefinition::Object(object)) = definition else {
            continue;
        };
        if object.name != "Query" && object.name != "Mutation" {
            continue;
        }
        for field in &object.fields {
            let directive = field.directives.iter().find(|d| d.name == "deprecated");
            let reason = match directive {
                Some(d) => d
                    .arguments
                    .iter()
                    .find_map(|(name, value)| match (name.as_str(), value) {
                        ("reason", Value::String(reason)) => Some(reason.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "deprecated".into()),
                None => match field.description.as_deref() {
                    Some(text) if text.trim_start().starts_with("[DEPRECATED]") => {
                        text.trim_start()["[DEPRECATED]".len()..].trim().to_string()
                    }
                    _ => continue,
                },
            };
            entries.push((
                field.name.clone(),
                reason.split_whitespace().collect::<Vec<_>>().join(" "),
            ));
        }
    }
    entries.sort();
    entries.dedup_by(|a, b| a.0 == b.0);
    let body: String = entries
        .iter()
        .map(|(field, reason)| format!("    ({field:?}, {reason:?}),\n"))
        .collect();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("deprecations.rs");
    fs::write(
        out,
        format!("pub const DEPRECATED_ROOT_FIELDS: &[(&str, &str)] = &[\n{body}];\n"),
    )
    .expect("write deprecation table");
}
