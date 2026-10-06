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
