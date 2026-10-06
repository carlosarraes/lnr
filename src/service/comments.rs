use super::{
    Service,
    page::{PageRequest, Reply, connection},
};
use crate::error::{AppError, Result};
use serde_json::{Value, json};
use std::collections::HashSet;
impl Service {
    pub async fn list_comments(&self, reference: &str, page: PageRequest) -> Result<Reply> {
        let mut after = page.after;
        let mut items = vec![];
        let mut seen = HashSet::new();
        if let Some(c) = &after {
            seen.insert(c.clone());
        }
        loop {
            let data = match self
                .query(
                    "Comments",
                    json!({"id":reference,"first":page.limit,"after":after}),
                )
                .await
            {
                Ok(v) => v,
                Err(mut e) => {
                    if !items.is_empty() {
                        e.data = Some(Value::Array(items));
                    }
                    return Err(e);
                }
            };
            if data["issue"].is_null() {
                return Err(AppError::new("not_found", "Issue not found"));
            }
            let (nodes, meta) = connection(&data["issue"]["comments"])?;
            items.extend(nodes);
            if !page.all || meta["has_more"] == false {
                return Ok(Reply {
                    data: items.into(),
                    meta: json!({"page":meta,"complete":meta["complete"]}),
                });
            }
            after = meta["end_cursor"].as_str().map(str::to_owned);
            if !seen.insert(after.clone().unwrap_or_default()) {
                let mut e = AppError::new("api", "Pagination cursor repeated");
                e.data = Some(items.into());
                return Err(e);
            }
        }
    }
    pub async fn list_relations(
        &self,
        reference: &str,
        page: PageRequest,
        inverse_after: Option<String>,
    ) -> Result<Reply> {
        let mut cursors = [page.after, inverse_after];
        let mut items = [vec![], vec![]];
        let mut metas = [json!({}), json!({})];
        let mut done = [false, false];
        let mut seen = [HashSet::new(), HashSet::new()];
        for i in 0..2 {
            if let Some(c) = &cursors[i] {
                seen[i].insert(c.clone());
            }
        }
        loop {
            let data=match self.query("Relations",json!({"id":reference,"first":page.limit,"after":cursors[0],"inverseAfter":cursors[1],"outgoing":!done[0],"incoming":!done[1]})).await{Ok(v)=>v,Err(mut e)=>{e.data=Some(json!({"relations":items[0],"inverse_relations":items[1]}));return Err(e)}};
            if data["issue"].is_null() {
                return Err(AppError::new("not_found", "Issue not found"));
            }
            for (i, key) in ["relations", "inverseRelations"].iter().enumerate() {
                if done[i] {
                    continue;
                }
                let (nodes, meta) = connection(&data["issue"][key])?;
                items[i].extend(nodes);
                metas[i] = meta;
                done[i] = metas[i]["has_more"] == false;
                cursors[i] = metas[i]["end_cursor"].as_str().map(str::to_owned);
                if page.all && !done[i] && !seen[i].insert(cursors[i].clone().unwrap_or_default()) {
                    return Err(AppError::new("api", "Pagination cursor repeated"));
                }
            }
            if !page.all || done.iter().all(|d| *d) {
                return Ok(Reply {
                    data: json!({"relations":items[0],"inverse_relations":items[1]}),
                    meta: json!({"collections":{"relations":metas[0],"inverse_relations":metas[1]},"complete":done.iter().all(|d|*d)}),
                });
            }
        }
    }
}
