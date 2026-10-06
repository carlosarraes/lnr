mod support;
use lnr::{
    api::{ApiClient, ClientOptions},
    auth::Credential,
    service::{Service, page::PageRequest, project_overview::OverviewFilter},
    tui::{
        data::{Loader, execute},
        state::*,
    },
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use support::*;
fn service(url: &str) -> Service {
    Service::new(
        ApiClient::with_options(
            Credential::new("test".into()).unwrap(),
            ClientOptions {
                endpoint: url.into(),
                ..Default::default()
            },
        )
        .unwrap(),
    )
}
#[tokio::test]
async fn project_issues_and_comments_use_selected_ids() {
    let s = Server::new(vec![
        (
            200,
            json!({"data":{"issues":page(json!([{ "id":"i", "title":"Issue"}]),false,None)}}),
        ),
        (
            200,
            json!({"data":{"issue":{"comments":page(json!([{ "id":"c","body":"Hi","user":null}]),false,None)}}}),
        ),
    ]);
    let service = service(&s.url);
    let r = execute(
        &service,
        RequestKind::Issues {
            id: "00000000-0000-0000-0000-000000000001".into(),
            page: PageRequest::default(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(r,Payload::Issues(p) if p.data.len()==1));
    let r = execute(
        &service,
        RequestKind::Comments {
            id: "i".into(),
            page: PageRequest {
                after: Some("next".into()),
                ..Default::default()
            },
        },
    )
    .await
    .unwrap();
    assert!(matches!(r,Payload::Comments(p) if p.data[0].user.is_none()));
    let req = s.requests.lock().unwrap();
    assert_eq!(
        req[0]["variables"]["filter"]["project"]["id"]["eq"],
        "00000000-0000-0000-0000-000000000001"
    );
    assert_eq!(req[1]["variables"]["id"], "i");
    assert_eq!(req[1]["variables"]["after"], "next");
}
#[tokio::test]
async fn superseded_request_and_drop_do_not_wait_for_http() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (started, ready) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let mut buf = [0; 8192];
        assert!(first.read(&mut buf).await.unwrap() > 0);
        started.send(()).unwrap();
        let (mut second, _) = listener.accept().await.unwrap();
        assert!(second.read(&mut buf).await.unwrap() > 0);
        let body = json!({"data":{"projects":page(json!([]),false,None)}}).to_string();
        second
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    let mut loader = Loader::new(Arc::new(service(&url)));
    let kind = RequestKind::Projects {
        filter: OverviewFilter::default(),
        page: PageRequest::default(),
    };
    loader.submit(Request {
        generation: 1,
        kind: kind.clone(),
    });
    tokio::time::timeout(Duration::from_secs(2), ready)
        .await
        .unwrap()
        .unwrap();
    loader.submit(Request {
        generation: 2,
        kind,
    });
    let r = tokio::time::timeout(Duration::from_secs(2), loader.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(r.generation, 2);
    assert!(r.result.is_ok());
    drop(loader);
    server.abort();
}
