use super::{Service,page::{Reply,connection}};
use crate::error::{AppError,Result};
use serde_json::json;
#[derive(Default)]pub struct ContextPageRequest {pub limit:u16,pub comments_after:Option<String>,pub children_after:Option<String>,pub relations_after:Option<String>,pub inverse_relations_after:Option<String>}
impl Service {
 pub async fn issue_context(&self,reference:&str,p:ContextPageRequest)->Result<Reply>{
  if !(1..=100).contains(&p.limit){return Err(AppError::input("Context limit must be 1..100"));}
  let data=self.query("IssueContext",json!({"id":reference,"limit":p.limit,"commentsAfter":p.comments_after,"childrenAfter":p.children_after,"relationsAfter":p.relations_after,"inverseRelationsAfter":p.inverse_relations_after})).await?;
  let mut issue=data.get("issue").filter(|v|!v.is_null()).cloned().ok_or_else(||AppError::new("not_found","Issue not found"))?;
  let mut collections=json!({});let mut complete=true;
  for (field,output) in [("children","children"),("comments","comments"),("relations","relations"),("inverseRelations","inverse_relations")] {
   let (nodes,meta)=connection(&issue[field])?;
   complete &= meta["complete"].as_bool().unwrap_or(false);
   issue.as_object_mut().ok_or_else(||AppError::new("api","Invalid issue"))?.remove(field);
   issue[output]=nodes.into();collections[output]=meta;
  }
  Ok(Reply{data:issue,meta:json!({"collections":collections,"complete":complete})})
 }
}
