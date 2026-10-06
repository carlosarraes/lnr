use super::{
    Service,
    page::{PageRequest, Pages, Reply},
};
use crate::{
    error::{AppError, Result},
    model::{Comment, RelationsPage},
};
use serde_json::{Value, json};
impl Service {
    pub async fn list_comments(
        &self,
        reference: &str,
        page: PageRequest,
    ) -> Result<Reply<Vec<Comment>>> {
        page.validate()?;
        let mut pages = Pages::new(page.after);
        loop {
            let data = match self
                .query(
                    "Comments",
                    json!({"id":reference,"first":page.limit,"after":pages.cursor}),
                )
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    if let Some(partial) = &e.data {
                        let _ = pages.ingest(&partial["issue"]["comments"]);
                    }
                    return Err(pages.fail(e));
                }
            };
            if data["issue"].is_null() {
                return Err(pages.fail(AppError::new("not_found", "Issue not found")));
            }
            if let Err(e) = pages.ingest(&data["issue"]["comments"]) {
                return Err(pages.fail(e));
            }
            if !page.all || !pages.has_more() {
                return pages.finish().decode();
            }
        }
    }
    pub async fn list_relations(
        &self,
        reference: &str,
        page: PageRequest,
        inverse_after: Option<String>,
    ) -> Result<Reply<RelationsPage>> {
        page.validate()?;
        let mut pages = [Pages::new(page.after), Pages::new(inverse_after)];
        let mut done = [false, false];
        loop {
            let result=self.query("Relations",json!({"id":reference,"first":page.limit,"after":pages[0].cursor,"inverseAfter":pages[1].cursor,"outgoing":!done[0],"incoming":!done[1]})).await;
            let (data, mut error) = match result {
                Ok(v) => (v, None),
                Err(mut e) => (e.data.take().unwrap_or(Value::Null), Some(e)),
            };
            if data["issue"].is_null() && error.is_none() {
                error = Some(AppError::new("not_found", "Issue not found"));
            }
            for (i, key) in ["relations", "inverseRelations"].iter().enumerate() {
                if done[i] {
                    continue;
                }
                if let Err(e) = pages[i].ingest(&data["issue"][key])
                    && error.is_none()
                {
                    error = Some(e);
                }
                done[i] = pages[i].meta["complete"] == true;
            }
            let data = json!({"relations":pages[0].items,"inverse_relations":pages[1].items});
            let meta = json!({"collections":{"relations":pages[0].meta,"inverse_relations":pages[1].meta},"complete":error.is_none() && done.iter().all(|d|*d)});
            if let Some(mut e) = error {
                e.data = Some(data);
                e.meta = (meta).into();
                return Err(e);
            }
            if !page.all || done.iter().all(|d| *d) {
                return Reply { data, meta }.decode();
            }
        }
    }
}
