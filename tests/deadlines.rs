use lnr::{
    api::{ApiClient, ClientOptions, OperationKind},
    auth::Credential,
};
use std::{
    io::Read,
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};
#[tokio::test]
async fn query_and_mutation_obey_total_deadlines() {
    for kind in [OperationKind::Query, OperationKind::Mutation] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0; 4096];
            let _ = stream.read(&mut buf);
            thread::sleep(Duration::from_millis(150));
        });
        let client = ApiClient::with_options(
            Credential::new("fixture".into()).unwrap(),
            ClientOptions {
                endpoint,
                timeout: Duration::from_millis(50),
                ..Default::default()
            },
        )
        .unwrap();
        let started = Instant::now();
        let error = client
            .execute("query { viewer { id } }", serde_json::json!({}), None, kind)
            .await
            .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            error.code,
            if kind == OperationKind::Query {
                "network"
            } else {
                "uncertain_mutation"
            }
        );
        handle.join().unwrap();
    }
}
