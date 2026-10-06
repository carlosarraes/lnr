use super::{
    Service,
    page::{Pages, Reply},
};
use crate::{
    error::{AppError, Result},
    model::{IssueContext, normalize},
};
use serde_json::{Value, json};
pub struct ContextPageRequest {
    pub limit: u16,
    pub comments_after: Option<String>,
    pub children_after: Option<String>,
    pub relations_after: Option<String>,
    pub inverse_relations_after: Option<String>,
}
impl Default for ContextPageRequest {
    fn default() -> Self {
        Self {
            limit: 20,
            comments_after: None,
            children_after: None,
            relations_after: None,
            inverse_relations_after: None,
        }
    }
}
impl Service {
    pub async fn issue_context(
        &self,
        reference: &str,
        p: ContextPageRequest,
    ) -> Result<Reply<IssueContext>> {
        if !(1..=100).contains(&p.limit) {
            return Err(AppError::input("Context limit must be 1..100"));
        }
        let result=self.query("IssueContext",json!({"id":reference,"limit":p.limit,"commentsAfter":p.comments_after,"childrenAfter":p.children_after,"relationsAfter":p.relations_after,"inverseRelationsAfter":p.inverse_relations_after})).await;
        let (data, mut error) = match result {
            Ok(v) => (v, None),
            Err(mut e) => (e.data.take().unwrap_or(Value::Null), Some(e)),
        };
        if data["issue"].is_null() {
            let mut e = error.unwrap_or_else(|| AppError::new("not_found", "Issue not found"));
            e.data = None;
            return Err(e);
        }
        let raw = &data["issue"];
        let mut issue = normalize(raw.clone());
        let mut collections = json!({});
        let mut complete = true;
        for (field, output, cursor) in [
            ("children", "children", p.children_after),
            ("comments", "comments", p.comments_after),
            ("relations", "relations", p.relations_after),
            (
                "inverseRelations",
                "inverse_relations",
                p.inverse_relations_after,
            ),
        ] {
            let mut pages = Pages::new(cursor);
            if let Err(e) = pages.ingest(&raw[field])
                && error.is_none()
            {
                error = Some(e);
            }
            complete &= pages.meta["complete"] == true;
            issue[output] = pages.items.into();
            collections[output] = pages.meta;
        }
        let meta = json!({"collections":collections,"complete":complete && error.is_none()});
        if let Some(mut e) = error {
            e.data = Some(issue);
            e.meta = (meta).into();
            return Err(e);
        }
        Reply { data: issue, meta }.decode()
    }
}
