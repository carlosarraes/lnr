//! Native OAuth authorization code flow with PKCE and rotating refresh tokens.
use super::{Credential, store};
use crate::{
    api::ApiClient,
    error::{AppError, Result},
    service::Service,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const TOKEN_URL: &str = "https://api.linear.app/oauth/token";
const PREFIX: &str = "lnr_oauth_v1.";

#[derive(Serialize, Deserialize)]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    client_id: String,
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    token_type: String,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random() -> Result<String> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| AppError::new("configuration", "OS random generator unavailable"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()).as_ref())
}
fn authorization_url(client_id: &str, redirect: &str, state: &str, verifier: &str) -> String {
    let mut url = reqwest::Url::parse("https://linear.app/oauth/authorize").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("response_type", "code"),
        ("scope", "read,write"),
        ("state", state),
        ("code_challenge", &challenge(verifier)),
        ("code_challenge_method", "S256"),
        ("prompt", "consent"),
        ("actor", "user"),
    ]);
    url.into()
}
fn callback_code(target: &str, expected: &str) -> Result<String> {
    let invalid = || {
        AppError::new(
            "authentication",
            "OAuth callback invalid or authorization denied; run lnr auth login again",
        )
    };
    if !target.starts_with("/callback?") {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| invalid())?;
    let pairs = url.query_pairs().collect::<Vec<_>>();
    let states = pairs
        .iter()
        .filter(|(k, _)| k == "state")
        .collect::<Vec<_>>();
    let codes = pairs
        .iter()
        .filter(|(k, _)| k == "code")
        .collect::<Vec<_>>();
    if url.path() != "/callback"
        || states.len() != 1
        || states[0].1 != expected
        || pairs.iter().any(|(k, _)| k == "error")
        || codes.len() != 1
        || codes[0].1.is_empty()
    {
        return Err(invalid());
    }
    Ok(codes[0].1.to_string())
}
async fn wait_callback(listener: tokio::net::TcpListener, state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let request = tokio::time::timeout(Duration::from_secs(5), async {
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") && head.len() < 8192 {
                head.push(stream.read_u8().await?);
            }
            Ok::<_, std::io::Error>(head)
        })
        .await;
        let Ok(Ok(head)) = request else {
            continue;
        };
        let text = String::from_utf8_lossy(&head);
        let mut parts = text.lines().next().unwrap_or("").split_whitespace();
        let method = parts.next();
        let target = parts.next().unwrap_or("");
        let result = if method == Some("GET") {
            callback_code(target, state)
        } else {
            Err(AppError::input("OAuth callback requires GET"))
        };
        let body = if result.is_ok() {
            "Authorization received. Return to your terminal to check that login completed."
        } else {
            "Invalid OAuth callback. Return to your terminal."
        };
        let status = if result.is_ok() {
            "200 OK"
        } else {
            "400 Bad Request"
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            stream.write_all(response.as_bytes()),
        )
        .await;
        if result.is_ok() {
            return result;
        }
        // Ignore stray browser requests and mismatched state; a matching denial ends the flow.
        if let Ok(url) = reqwest::Url::parse(&format!("http://127.0.0.1{target}"))
            && url.query_pairs().any(|(k, v)| k == "state" && v == state)
        {
            return result;
        }
    }
}
fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .map_err(|_| AppError::new("network", "Cannot initialize OAuth HTTP client"))
}
async fn token_request(
    client: &reqwest::Client,
    endpoint: &str,
    params: &[(&str, &str)],
) -> Result<TokenResponse> {
    let mut response = client
        .post(endpoint)
        .form(params)
        .send()
        .await
        .map_err(|_| {
            AppError::new(
                "network",
                "OAuth token request failed; retry login or the command",
            )
        })?;
    if !response.status().is_success() {
        return Err(AppError::new(
            "authentication",
            format!(
                "OAuth token request failed (HTTP {}); run lnr auth login --replace",
                response.status().as_u16()
            ),
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::new("network", "OAuth response interrupted; retry"))?
    {
        if bytes.len() + chunk.len() > 65536 {
            return Err(AppError::new("authentication", "OAuth response too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let token: TokenResponse = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::new("authentication", "Invalid OAuth token response"))?;
    if token.access_token.trim().is_empty()
        || token.refresh_token.trim().is_empty()
        || token.expires_in == 0
        || !token.token_type.eq_ignore_ascii_case("Bearer")
    {
        return Err(AppError::new(
            "authentication",
            "OAuth response is missing a valid access token, refresh token, expiry, or Bearer type",
        ));
    }
    Ok(token)
}
impl Session {
    fn from_response(token: TokenResponse, client_id: &str) -> Result<Self> {
        Ok(Self {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at: now()
                .checked_add(token.expires_in)
                .ok_or_else(|| AppError::new("authentication", "Invalid OAuth expiry"))?,
            client_id: client_id.into(),
        })
    }
    async fn refresh(&mut self, client: &reqwest::Client, endpoint: &str) -> Result<()> {
        let token = token_request(
            client,
            endpoint,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", &self.client_id),
                ("refresh_token", &self.refresh_token),
            ],
        )
        .await?;
        *self = Self::from_response(token, &self.client_id)?;
        Ok(())
    }
    fn encode(&self) -> Result<String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| AppError::new("configuration", "Cannot encode OAuth credentials"))?;
        Ok(format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes)))
    }
}
pub async fn resolve_token(workspace: &str, key: String) -> Result<Credential> {
    let Some(encoded) = key.strip_prefix(PREFIX) else {
        return Credential::new(key);
    };
    let invalid = || {
        AppError::new(
            "authentication",
            "Invalid stored OAuth credentials; run lnr auth login --replace",
        )
    };
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| invalid())?;
    let mut session: Session = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if session.client_id.is_empty() || session.refresh_token.is_empty() {
        return Err(invalid());
    }
    if now().saturating_add(300) >= session.expires_at {
        session.refresh(&http_client()?, TOKEN_URL).await?;
        // Caller holds the cross-process lock until the new rotating token is persisted.
        store::write(workspace, &session.encode()?)?;
    }
    Credential::bearer(session.access_token)
}
pub async fn login(
    client_id: &str,
    no_browser: bool,
    port: u16,
    timeout: u16,
    workspace: Option<&str>,
    replace: bool,
) -> Result<Value> {
    if client_id.trim().is_empty() {
        return Err(AppError::input("OAuth client ID must not be empty"));
    }
    if std::env::var_os("LINEAR_API_KEY").is_some() {
        return Err(AppError::input(
            "Unset LINEAR_API_KEY before OAuth login so subsequent commands use the stored OAuth session",
        ));
    }
    let listener=tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST,port)).await.map_err(|_|AppError::new("configuration",format!("Cannot bind OAuth callback port {port}; close the other login process or choose a registered --port")))?;
    let redirect = format!("http://127.0.0.1:{port}/callback");
    let verifier = random()?;
    let state = random()?;
    let url = authorization_url(client_id, &redirect, &state, &verifier);
    eprintln!(
        "Authorize lnr in your browser:\n{url}\nWaiting up to {timeout}s for {redirect}. For SSH, forward this port to the machine running lnr."
    );
    if !no_browser {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        match std::process::Command::new(program)
            .arg(&url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(_) => eprintln!("Could not open a browser; open the URL above manually."),
        }
    }
    let code = tokio::time::timeout(
        Duration::from_secs(timeout.into()),
        wait_callback(listener, &state),
    )
    .await
    .map_err(|_| {
        AppError::new(
            "authentication",
            "OAuth authorization timed out; run lnr auth login again",
        )
    })??;
    let token = token_request(
        &http_client()?,
        TOKEN_URL,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("redirect_uri", &redirect),
            ("code", &code),
            ("code_verifier", &verifier),
        ],
    )
    .await?;
    let session = Session::from_response(token, client_id)?;
    let status = Service::new(ApiClient::new(Credential::bearer(
        session.access_token.clone(),
    )?)?)
    .status()
    .await?;
    let slug = status
        .organization
        .as_ref()
        .map(|o| o.url_key.as_str())
        .ok_or_else(|| AppError::new("authentication", "OAuth login returned no workspace"))?;
    if workspace.is_some_and(|ws| ws != slug) {
        return Err(AppError::new(
            "authentication",
            "Authorized a different workspace than --workspace; retry and select the requested workspace",
        ));
    }
    super::save(slug, &session.encode()?, replace).await?;
    Ok(
        json!({"workspace":slug,"authenticated":true,"auth_type":"oauth","expires_at":session.expires_at}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_matches_rfc7636_vector() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn callback_requires_matching_state_and_single_code() {
        assert_eq!(
            callback_code("/callback?code=abc%2Bdef&state=expected", "expected").unwrap(),
            "abc+def"
        );
        for target in [
            "/callback?code=abc&state=wrong",
            "/callback?code=abc",
            "/callback?state=expected&error=access_denied",
            "/callback?state=expected&code=a&code=b",
            "/other?state=expected&code=a",
            "/callback?state=expected&code=",
        ] {
            assert!(callback_code(target, "expected").is_err(), "{target}");
        }
    }
    async fn token_server(status: u16, body: Value) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                head.push(stream.read_u8().await.unwrap());
            }
            let text = String::from_utf8(head).unwrap();
            let length = text
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|s| s.parse::<usize>().ok())
                })
                .unwrap();
            assert!(
                text.to_lowercase()
                    .contains("content-type: application/x-www-form-urlencoded")
            );
            let mut data = vec![0u8; length];
            stream.read_exact(&mut data).await.unwrap();
            let body = body.to_string();
            stream.write_all(format!("HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            String::from_utf8(data).unwrap()
        });
        (url, task)
    }
    #[tokio::test]
    async fn refresh_rotates_both_tokens_and_preserves_client_id() {
        let (url,request)=token_server(200,json!({"access_token":"new-access","refresh_token":"new-refresh","expires_in":86400,"token_type":"Bearer"})).await;
        let mut session = Session {
            access_token: "old-access".into(),
            refresh_token: "old+refresh".into(),
            expires_at: 0,
            client_id: "client".into(),
        };
        session
            .refresh(&http_client().unwrap(), &url)
            .await
            .unwrap();
        assert_eq!(session.access_token, "new-access");
        assert_eq!(session.refresh_token, "new-refresh");
        assert_eq!(session.client_id, "client");
        assert!(session.expires_at > now() + 86000);
        let form = request.await.unwrap();
        let pairs = reqwest::Url::parse(&format!("http://localhost/?{form}"))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(pairs["grant_type"], "refresh_token");
        assert_eq!(pairs["refresh_token"], "old+refresh");
        assert_eq!(pairs["client_id"], "client");
        assert!(!pairs.contains_key("client_secret"));
    }
    #[tokio::test]
    async fn rejected_token_response_never_exposes_response_secrets() {
        let (url, request) = token_server(400, json!({"error":"test-secret-refresh"})).await;
        let error = token_request(
            &http_client().unwrap(),
            &url,
            &[("grant_type", "refresh_token")],
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.code, "authentication");
        assert!(!error.message.contains("test-secret-refresh"));
        request.await.unwrap();
    }
    #[tokio::test]
    async fn unexpired_stored_oauth_uses_bearer_without_network() {
        let session = Session {
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            expires_at: now() + 86400,
            client_id: "client".into(),
        };
        let credential = resolve_token("workspace", session.encode().unwrap())
            .await
            .unwrap();
        assert_eq!(credential.authorization(), "Bearer access");
        assert_eq!(credential.secret(), "access");
        assert!(!format!("{credential:?}").contains("access"));
    }
    #[tokio::test]
    async fn callback_server_ignores_stray_requests() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let callback = tokio::spawn(async move { wait_callback(listener, "state").await });
        let client = http_client().unwrap();
        let bad = client
            .get(format!("http://{address}/favicon.ico"))
            .send()
            .await
            .unwrap();
        assert_eq!(bad.status(), 400);
        let good = client
            .get(format!("http://{address}/callback?state=state&code=good"))
            .send()
            .await
            .unwrap();
        assert_eq!(good.status(), 200);
        assert_eq!(callback.await.unwrap().unwrap(), "good");
    }
    #[tokio::test]
    async fn incomplete_token_response_is_rejected() {
        let (url,request)=token_server(200,json!({"access_token":"access","refresh_token":"","expires_in":86400,"token_type":"Bearer"})).await;
        assert!(
            token_request(
                &http_client().unwrap(),
                &url,
                &[("grant_type", "authorization_code")]
            )
            .await
            .is_err()
        );
        request.await.unwrap();
    }
    #[tokio::test]
    async fn failed_refresh_keeps_previous_session() {
        let (url, request) = token_server(400, json!({"error":"invalid_grant"})).await;
        let mut session = Session {
            access_token: "old-access".into(),
            refresh_token: "old-refresh".into(),
            expires_at: 0,
            client_id: "client".into(),
        };
        assert!(
            session
                .refresh(&http_client().unwrap(), &url)
                .await
                .is_err()
        );
        assert_eq!(session.refresh_token, "old-refresh");
        assert_eq!(session.access_token, "old-access");
        request.await.unwrap();
    }
    #[tokio::test]
    async fn graphql_sends_bearer_and_redacts_raw_access_token() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                head.push(stream.read_u8().await.unwrap());
            }
            let head = String::from_utf8(head).unwrap();
            assert!(
                head.to_lowercase()
                    .contains("authorization: bearer secret-access")
            );
            let length = head
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|s| s.parse::<usize>().ok())
                })
                .unwrap();
            let mut body = vec![0u8; length];
            stream.read_exact(&mut body).await.unwrap();
            let body = r#"{"data":{"echo":"secret-access"}}"#;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let client = ApiClient::with_options(
            Credential::bearer("secret-access".into()).unwrap(),
            crate::api::ClientOptions {
                endpoint,
                ..Default::default()
            },
        )
        .unwrap();
        let reply = client
            .execute(
                "query { viewer { id } }",
                json!({}),
                None,
                crate::api::OperationKind::Query,
            )
            .await
            .unwrap();
        assert_eq!(reply["echo"], "[redacted]");
        server.await.unwrap();
    }
}
