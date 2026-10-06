mod support;
use lnr::{
    api::{ApiClient, ClientOptions},
    auth::Credential,
    service::{Service, page::PageRequest, project_overview::OverviewFilter},
};
use serde_json::json;
use support::*;
fn service(s: &Server) -> Service {
    Service::new(
        ApiClient::with_options(
            Credential::new("test".into()).unwrap(),
            ClientOptions {
                endpoint: s.url.clone(),
                ..Default::default()
            },
        )
        .unwrap(),
    )
}
#[tokio::test]
async fn overview_filters_are_intersected_and_progress_is_authoritative() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"projects":page(json!([{"id":"p","name":"P","progress":0.5,"lead":{"id":"u","isMe":true},"teams":page(json!([]),false,None),"members":{"nodes":[{"id":"u"}]},"issues":{"nodes":[{"id":"i"}]}}]),true,Some("next"))}}),
    )]);
    let result = service(&s)
        .overview_projects(
            OverviewFilter {
                mine: true,
                team_ids: vec!["t1".into(), "t2".into()],
                status_ids: vec!["s1".into(), "s2".into()],
                involvement: vec!["lead".into()],
                query: "Api".into(),
            },
            PageRequest::default(),
        )
        .await
        .unwrap();
    assert_eq!(result.data[0].project.progress, Some(0.5));
    assert_eq!(result.data[0].project.target_date, None);
    assert_eq!(
        result.data[0].involvement,
        vec!["lead", "member", "assignee"]
    );
    assert_eq!(result.meta["page"]["end_cursor"], "next");
    let r = s.requests.lock().unwrap();
    let f = &r[0]["variables"]["filter"];
    assert_eq!(
        f["accessibleTeams"]["some"]["id"]["in"],
        json!(["t1", "t2"])
    );
    assert_eq!(f["status"]["id"]["in"], json!(["s1", "s2"]));
    assert_eq!(f["name"]["containsIgnoreCase"], "Api");
    assert_eq!(f["and"][0]["or"].as_array().unwrap().len(), 3);
    assert_eq!(f["and"][1]["or"].as_array().unwrap().len(), 1);
}
#[tokio::test]
async fn all_projects_and_later_page_preserve_missing_values() {
    let s = Server::new(vec![(
        200,
        json!({"data":{"projects":page(json!([{"id":"p","teams":page(json!([]),false,None),"members":{"nodes":[]},"issues":{"nodes":[]}}]),false,None)}}),
    )]);
    let r = service(&s)
        .overview_projects(
            OverviewFilter::default(),
            PageRequest {
                after: Some("cursor".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.data[0].project.progress, None);
    assert!(r.data[0].involvement.is_empty());
    let req = s.requests.lock().unwrap();
    assert_eq!(req[0]["variables"]["filter"], json!({}));
    assert_eq!(req[0]["variables"]["after"], "cursor");
}
#[tokio::test]
async fn catalogs_and_project_teams_follow_pages() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"viewer":{"id":"u"},"organization":{"urlKey":"demo"}}}),
        ),
        (
            200,
            json!({"data":{"teams":page(json!([{"id":"t1","name":"A"}]),true,Some("t"))}}),
        ),
        (
            200,
            json!({"data":{"teams":page(json!([{"id":"t2","name":"B"}]),false,None)}}),
        ),
        (
            200,
            json!({"data":{"projectStatuses":page(json!([{"id":"s1","name":"Same"}]),true,Some("s"))}}),
        ),
        (
            200,
            json!({"data":{"projectStatuses":page(json!([{"id":"s2","name":"Same"}]),false,None)}}),
        ),
    ]);
    let r = service(&s).overview_catalog().await.unwrap();
    assert_eq!(r.teams.len(), 2);
    assert_eq!(r.statuses.len(), 2);
    assert_ne!(r.statuses[0].id, r.statuses[1].id);
}

#[tokio::test]
async fn partial_api_error_keeps_typed_available_projects() {
    let node = json!({"id":"p","progress":null,"teams":page(json!([]),false,None),"members":{"nodes":[]},"issues":{"nodes":[]}});
    let s = Server::new(vec![(
        200,
        json!({"data":{"projects":page(json!([node]),true,Some("more"))},"errors":[{"message":"partial"}]}),
    )]);
    let error = service(&s)
        .overview_projects(OverviewFilter::default(), PageRequest::default())
        .await
        .unwrap_err();
    let rows: Vec<lnr::service::project_overview::OverviewProject> =
        serde_json::from_value(error.data.unwrap()).unwrap();
    assert_eq!(rows[0].project.id, "p");
}
#[tokio::test]
async fn nested_teams_are_not_silently_truncated() {
    let node = json!({"id":"p","teams":page(json!([{"id":"t1"}]),true,Some("team2")),"members":{"nodes":[{"id":"me"}]},"issues":{"nodes":[]}});
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"projects":page(json!([node]),false,None)}}),
        ),
        (
            200,
            json!({"data":{"project":{"teams":page(json!([{"id":"t2"}]),false,None)}}}),
        ),
    ]);
    let reply = service(&s)
        .overview_projects(OverviewFilter::default(), PageRequest::default())
        .await
        .unwrap();
    assert_eq!(reply.data[0].teams.len(), 2);
    assert_eq!(reply.data[0].involvement, vec!["member"]);
    assert_eq!(s.requests.lock().unwrap()[1]["variables"]["after"], "team2");
}
