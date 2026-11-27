use super::{
    Service,
    issues::IssueFilter,
    page::{PageRequest, Reply},
};
use crate::{error::Result, model::Issue};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

const GATE_BATCH: usize = 80;

impl Service {
    /// Issues with no unfinished blocker: neither a native blocked-by relation nor an
    /// `after KEY-N` description gate may point at an issue whose state is not completed.
    pub async fn ready_issues(
        &self,
        f: IssueFilter,
        page: PageRequest,
    ) -> Result<Reply<Vec<Issue>>> {
        let filter = self.issue_filter(f).await?;
        let reply = self.list("ReadyIssues", "issues", filter, page).await?;
        let candidates = match reply.data {
            Value::Array(items) => items,
            _ => vec![],
        };
        let gates: Vec<_> = candidates
            .iter()
            .map(|i| description_gates(i["description"].as_str().unwrap_or_default()))
            .collect();
        let mut wanted: Vec<String> = gates.iter().flatten().cloned().collect();
        wanted.sort();
        wanted.dedup();
        let states = self.gate_states(&wanted).await?;
        let mut ready = vec![];
        let mut blocked = vec![];
        for (mut issue, gates) in candidates.into_iter().zip(gates) {
            let mut blockers = vec![];
            let incoming = &issue["inverse_relations"];
            for relation in incoming["nodes"].as_array().into_iter().flatten() {
                if relation["type"] != "blocks" {
                    continue;
                }
                let state = &relation["issue"]["state"]["type"];
                if state != "completed" {
                    blockers.push(json!({"identifier":relation["issue"]["identifier"],"source":"relation","state":state}));
                }
            }
            if incoming["page_info"]["has_next_page"] != false {
                blockers.push(
                    json!({"identifier":null,"source":"relation","state":null,"unverified":true}),
                );
            }
            for gate in gates {
                let state = states.get(&gate);
                if state.map(String::as_str) != Some("completed") {
                    blockers.push(json!({"identifier":gate,"source":"description","state":state}));
                }
            }
            if let Some(map) = issue.as_object_mut() {
                map.remove("inverse_relations");
            }
            if blockers.is_empty() {
                ready.push(issue);
            } else {
                blocked.push(
                    json!({"id":issue["id"],"identifier":issue["identifier"],"blockers":blockers}),
                );
            }
        }
        let mut meta = reply.meta;
        meta["blocked"] = blocked.into();
        Reply {
            data: ready.into(),
            meta,
        }
        .decode()
    }

    /// Map gate identifiers to their state type, querying one batch of identifiers at a time.
    async fn gate_states(&self, identifiers: &[String]) -> Result<HashMap<String, String>> {
        let mut states = HashMap::new();
        for chunk in identifiers.chunks(GATE_BATCH) {
            let mut teams: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
            for id in chunk {
                if let Some((key, number)) = id.rsplit_once('-')
                    && let Ok(number) = number.parse()
                {
                    teams.entry(key).or_default().push(number);
                }
            }
            let filter = json!({"or":teams.iter().map(|(key, numbers)| json!({"team":{"key":{"eq":key}},"number":{"in":numbers}})).collect::<Vec<_>>()});
            let reply = self
                .list(
                    "Issues",
                    "issues",
                    filter,
                    PageRequest {
                        limit: 100,
                        after: None,
                        all: true,
                    },
                )
                .await
                .map_err(|mut e| {
                    e.data = None;
                    e.meta = Box::new(json!({"complete":false}));
                    e
                })?;
            for issue in reply.data.as_array().into_iter().flatten() {
                if let (Some(id), Some(state)) = (
                    issue["identifier"].as_str(),
                    issue["state"]["type"].as_str(),
                ) {
                    states.insert(id.to_ascii_uppercase(), state.to_owned());
                }
            }
        }
        Ok(states)
    }
}

/// Identifiers following `after ` or `Build after ` (case-insensitive), uppercased, in order.
pub fn description_gates(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut gates: Vec<String> = vec![];
    let mut from = 0;
    while let Some(at) = lower[from..].find("after ") {
        let start = from + at;
        from = start + "after ".len();
        if start > 0 && lower.as_bytes()[start - 1].is_ascii_alphanumeric() {
            continue;
        }
        if let Some(id) = identifier(&text[from..])
            && !gates.contains(&id)
        {
            gates.push(id);
        }
    }
    gates
}

fn identifier(s: &str) -> Option<String> {
    let key = s.bytes().take_while(u8::is_ascii_alphanumeric).count();
    if key == 0 || !s.as_bytes()[0].is_ascii_alphabetic() {
        return None;
    }
    let rest = s[key..].strip_prefix('-')?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0).then(|| format!("{}-{}", s[..key].to_ascii_uppercase(), &rest[..digits]))
}
