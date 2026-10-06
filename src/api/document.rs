use crate::error::{AppError, Result};
use graphql_parser::query::{
    Definition, Document, OperationDefinition, Selection, SelectionSet, parse_query,
};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};
pub fn selected(name: &str) -> Result<&'static str> {
    static DOCUMENTS: OnceLock<HashMap<String, String>> = OnceLock::new();
    let docs = DOCUMENTS.get_or_init(|| {
        let doc =
            parse_query::<String>(super::checked::DOCUMENT).expect("compile-time checked GraphQL");
        let mut docs = HashMap::new();
        for definition in &doc.definitions {
            let (name, selection) = match definition {
                Definition::Operation(OperationDefinition::Query(q)) => {
                    (q.name.as_ref(), &q.selection_set)
                }
                Definition::Operation(OperationDefinition::Mutation(m)) => {
                    (m.name.as_ref(), &m.selection_set)
                }
                _ => continue,
            };
            let Some(name) = name else { continue };
            let mut needed = HashSet::new();
            spreads(selection, &mut needed);
            loop {
                let before = needed.len();
                for d in &doc.definitions {
                    if let Definition::Fragment(f) = d
                        && needed.contains(&f.name)
                    {
                        spreads(&f.selection_set, &mut needed);
                    }
                }
                if before == needed.len() {
                    break;
                }
            }
            let mut definitions = vec![definition.clone()];
            definitions.extend(
                doc.definitions
                    .iter()
                    .filter(|d| matches!(d,Definition::Fragment(f) if needed.contains(&f.name)))
                    .cloned(),
            );
            docs.insert(name.clone(), Document { definitions }.to_string());
        }
        docs
    });
    docs.get(name)
        .map(String::as_str)
        .ok_or_else(|| AppError::input("Unknown built-in operation"))
}
fn spreads(selection: &SelectionSet<'_, String>, names: &mut HashSet<String>) {
    for item in &selection.items {
        match item {
            Selection::FragmentSpread(f) => {
                names.insert(f.fragment_name.clone());
            }
            Selection::Field(f) => spreads(&f.selection_set, names),
            Selection::InlineFragment(f) => spreads(&f.selection_set, names),
        }
    }
}
