#![allow(dead_code)]
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
pub struct Server {
    pub url: String,
    pub requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Server {
    pub fn new(replies: Vec<(u16, Value)>) -> Self {
        Self::with_headers(replies, String::new())
    }
    pub fn with_headers(replies: Vec<(u16, Value)>, response_headers: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(vec![]));
        let stop = Arc::new(AtomicBool::new(false));
        let r = requests.clone();
        let s = stop.clone();
        let handle = thread::spawn(move || {
            let mut replies = replies.into_iter();
            'accept: while !s.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = vec![];
                let mut buf = [0; 4096];
                let split = loop {
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        eprintln!("fixture: ignoring a connection closed before HTTP headers");
                        continue 'accept;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(p) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break p + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&bytes[..split]);
                let len = headers
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < split + len {
                    let n = stream.read(&mut buf).unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                }
                r.lock()
                    .unwrap()
                    .push(serde_json::from_slice(&bytes[split..]).unwrap());
                let (status, body) = replies.next().unwrap_or((
                    500,
                    serde_json::json!({"errors":[{"message":"unexpected request"}]}),
                ));
                let body = body.to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{response_headers}\r\n{body}",
                    body.len()
                );
            }
        });
        Self {
            url,
            requests,
            stop,
            handle: Some(handle),
        }
    }
    pub fn run(&self, args: &[&str]) -> Output {
        self.run_input(args, None, true)
    }
    pub fn run_input(&self, args: &[&str], input: Option<&str>, key: bool) -> Output {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Command::new(env!("CARGO_BIN_EXE_lnr"));
        c.args(args)
            .env_remove("LINEAR_API_KEY")
            .env("XDG_CONFIG_HOME", dir.path())
            .env("LNR_API_URL", &self.url)
            .env("NO_PROXY", "127.0.0.1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());
        if key {
            c.env("LINEAR_API_KEY", "secret-test-key");
        }
        let mut child = c.spawn().unwrap();
        if let Some(input) = input {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
        } else {
            drop(child.stdin.take());
        }
        child.wait_with_output().unwrap()
    }
    pub fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.take().unwrap().join().unwrap();
    }
}
pub fn value(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout={} stderr={}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )
    })
}
pub fn page(nodes: Value, more: bool, cursor: Option<&str>) -> Value {
    serde_json::json!({"nodes":nodes,"pageInfo":{"hasNextPage":more,"endCursor":cursor}})
}
