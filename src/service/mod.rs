use crate::{api::{ApiClient,OperationKind},error::{AppError,Result}};
use serde_json::{Value,json};
pub mod page;
pub mod issues;
pub mod resolve;
pub struct Service{pub(crate) client:ApiClient, resolved:tokio::sync::Mutex<std::collections::HashMap<String,String>>}
impl Service {
 pub fn new(client:ApiClient)->Self{Self{client,resolved:Default::default()}}
 pub async fn status(&self)->Result<Value>{self.query("AuthStatus",json!({})).await}
 pub async fn raw_api(&self,document:String,variables:Value,name:Option<String>)->Result<Value>{
  use graphql_parser::query::{Definition,OperationDefinition,parse_query};
  if !variables.is_object(){return Err(AppError::input("GraphQL variables must be a JSON object"));}
  let doc=parse_query::<String>(&document).map_err(|_|AppError::input("Invalid GraphQL document"))?;
  let ops:Vec<_>=doc.definitions.iter().filter_map(|d|if let Definition::Operation(op)=d{Some(op)}else{None}).collect();
  let op=if let Some(name)=name.as_deref(){ops.iter().find(|op|match op{OperationDefinition::Query(q)=>q.name.as_deref()==Some(name),OperationDefinition::Mutation(q)=>q.name.as_deref()==Some(name),OperationDefinition::Subscription(q)=>q.name.as_deref()==Some(name),_=>false}).copied()}else if ops.len()==1{ops.first().copied()}else{None}.ok_or_else(||AppError::input("Select exactly one GraphQL operation with --operation-name"))?;
  let kind=match op{OperationDefinition::Mutation(_)=>OperationKind::Mutation,OperationDefinition::Subscription(_)=>return Err(AppError::input("Subscriptions are not supported")),_=>OperationKind::Query};
  self.client.execute(&document,variables,name.as_deref(),kind).await
 }
}
