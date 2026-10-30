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
    pub async fn add_comment(
        &self,
        reference: &str,
        body: String,
        mentions: Vec<String>,
    ) -> Result<Comment> {
        validate_comment_body(&body)?;
        let id = self.resolve("issue", reference, None).await?;
        let (body, mentioned_users) = self.mention_body(body, mentions).await?;
        let mut comment: Comment = crate::model::decode(
            self.mutate(
                "AddComment",
                "commentCreate",
                Some("comment"),
                json!({"input":{"issueId":id,"body":body}}),
            )
            .await?,
        )?;
        comment.mentioned_users = mentioned_users;
        Ok(comment)
    }

    pub async fn edit_comment(
        &self,
        id: &str,
        body: String,
        mentions: Vec<String>,
    ) -> Result<Comment> {
        validate_comment_id(id)?;
        validate_comment_body(&body)?;
        let (body, mentioned_users) = self.mention_body(body, mentions).await?;
        let mut comment: Comment = crate::model::decode(
            self.mutate(
                "EditComment",
                "commentUpdate",
                Some("comment"),
                json!({"id":id,"input":{"body":body}}),
            )
            .await?,
        )?;
        comment.mentioned_users = mentioned_users;
        Ok(comment)
    }

    pub async fn delete_comment(&self, id: &str) -> Result<Value> {
        validate_comment_id(id)?;
        self.mutate("DeleteComment", "commentDelete", None, json!({"id":id}))
            .await
    }

    async fn mention_body(
        &self,
        body: String,
        mentions: Vec<String>,
    ) -> Result<(String, Vec<crate::model::MentionedUser>)> {
        let mut users: Vec<crate::model::MentionedUser> = vec![];
        for reference in mentions {
            let id = self.resolve("user", &reference, None).await?;
            if users.iter().any(|u| u.id == id) {
                continue;
            }
            let data = self.query("MentionUser", json!({"id":id})).await?;
            let user: crate::model::MentionedUser = crate::model::decode(data["user"].clone())?;
            users.push(user);
        }
        if users.is_empty() {
            return Ok((body, users));
        }
        let urls = users
            .iter()
            .map(|u| u.url.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Ok((format!("{urls}\n\n{body}"), users))
    }

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

fn validate_comment_id(id: &str) -> Result<()> {
    if id.trim().is_empty() {
        return Err(AppError::input("Comment ID must not be empty"));
    }
    Ok(())
}

fn validate_comment_body(body: &str) -> Result<()> {
    if body.trim().is_empty() {
        return Err(AppError::input("Comment body must not be empty"));
    }
    Ok(())
}
