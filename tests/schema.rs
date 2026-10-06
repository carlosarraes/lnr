mod support;
use support::*;
#[test]
fn discovery_needs_no_credentials() {
    let s = Server::new(vec![]);
    let o = s.run_input(&["schema"], None, false);
    assert!(o.status.success(), "{:?}", value(&o));
    let v = value(&o);
    let commands = v["data"]["commands"].as_array().unwrap();
    assert!(commands.iter().any(|c| c["path"] == "lnr issue context"));
    assert!(v["data"]["output_schema"].is_object());
    assert_eq!(s.count(), 0);
}
#[test]
fn real_outputs_validate_against_discovered_schemas() {
    use serde_json::json;
    let discovery = Server::new(vec![]).run(&["schema"]);
    let schemas = value(&discovery);
    for (args, body, output) in [
        (
            vec!["issue", "list"],
            json!({"data":{"issues":page(json!([{"id":"i","title":"Fix"}]),false,None)}}),
            "issues",
        ),
        (
            vec!["project", "list"],
            json!({"data":{"projects":page(json!([{"id":"p","targetDate":"2026-12-01"}]),false,None)}}),
            "projects",
        ),
        (
            vec!["issue", "context", "ENG-1"],
            json!({"data":{"issue":{"id":"i","children":page(json!([]),false,None),"comments":page(json!([]),false,None),"relations":page(json!([]),false,None),"inverseRelations":page(json!([]),false,None)}}}),
            "issue_context",
        ),
    ] {
        let s = Server::new(vec![(200, body)]);
        let o = s.run(&args);
        assert!(o.status.success(), "{:?}", value(&o));
        let schema = &schemas["data"]["outputs"][output];
        let validator = jsonschema::validator_for(schema).unwrap();
        let instance = value(&o);
        assert!(
            validator.is_valid(&instance),
            "{output}: {:?}",
            validator
                .iter_errors(&instance)
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
        );
    }
}
