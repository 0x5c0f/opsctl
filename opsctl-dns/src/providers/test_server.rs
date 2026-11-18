#![cfg(test)]

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

#[derive(Debug, Clone)]
pub(crate) struct MockResponse {
    pub status: u16,
    pub body: &'static str,
}

impl MockResponse {
    pub fn ok(body: &'static str) -> Self {
        Self { status: 200, body }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub authorization: Option<String>,
    pub body: String,
}

pub(crate) struct MockServer {
    base_url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    handle: Option<JoinHandle<()>>,
}

impl MockServer {
    pub fn new(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定测试端口失败");
        let addr = listener.local_addr().expect("读取测试端口失败");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let reqs = requests.clone();
        let handle = std::thread::spawn(move || {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut reader = BufReader::new(&stream);
                let mut request_line = String::new();
                let _ = reader.read_line(&mut request_line);
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let path = parts.next().unwrap_or("").to_string();

                let mut authorization = None;
                let mut content_length = 0usize;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        let value = value.trim();
                        if name.eq_ignore_ascii_case("authorization") {
                            authorization = Some(value.to_string());
                        }
                        if name.eq_ignore_ascii_case("content-length") {
                            content_length = value.parse().unwrap_or(0);
                        }
                    }
                }
                let mut body = vec![0u8; content_length];
                let _ = reader.read_exact(&mut body);
                reqs.lock().expect("记录请求锁失败").push(RecordedRequest {
                    method,
                    path,
                    authorization,
                    body: String::from_utf8_lossy(&body).to_string(),
                });

                let reason = if response.status == 200 {
                    "OK"
                } else {
                    "Error"
                };
                let header = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.status,
                    reason,
                    response.body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(response.body.as_bytes());
                let _ = stream.flush();
            }
        });
        Self {
            base_url: format!("http://{addr}"),
            requests,
            handle: Some(handle),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("读取请求锁失败").clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
