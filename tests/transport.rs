mod support;
use support::*;use serde_json::json;
#[test] fn graphql_partial_error_is_not_success(){
 let s=Server::new(vec![(200,json!({"data":{"viewer":{"id":"u"}},"errors":[{"message":"denied"}]}))]);
 let o=s.run(&["auth","status"]);assert_eq!(o.status.code(),Some(8));assert_eq!(value(&o)["data"]["viewer"]["id"],"u");assert_eq!(s.count(),1);
}
#[test] fn query_retries_then_succeeds(){
 let s=Server::new(vec![(503,json!({})),(200,json!({"data":{"viewer":{"id":"u","name":"Me"},"organization":{"urlKey":"test"}}}))]);
 let o=s.run(&["auth","status"]);assert!(o.status.success(),"{:?}",value(&o));assert_eq!(s.count(),2);
}
#[test] fn mutation_never_retries(){
 let s=Server::new(vec![(503,json!({}))]);let o=s.run_input(&["api","--query-file","-"],Some("mutation { issueDelete(id: \"x\") { success } }"),true);
 assert_eq!(o.status.code(),Some(8));assert_eq!(value(&o)["error"]["code"],"uncertain_mutation");assert_eq!(s.count(),1);
}
#[test] fn exhausted_rate_limit(){
 let s=Server::new(vec![(429,json!({})),(429,json!({})),(429,json!({}))]);let o=s.run(&["auth","status"]);assert_eq!(o.status.code(),Some(6));assert_eq!(s.count(),3);
}
#[test] fn secrets_are_redacted(){
 let s=Server::new(vec![(200,json!({"errors":[{"message":"bad secret-test-key"}]}))]);let o=s.run(&["auth","status"]);
 assert!(!String::from_utf8_lossy(&o.stdout).contains("secret-test-key"));assert!(!String::from_utf8_lossy(&o.stderr).contains("secret-test-key"));
}
