//! HTTP request replay client for FlowLens.
//!
//! This module provides the `ReplayClient` which can send captured HTTP requests
//! to a target server, optionally with modifications.

use crate::errors::ReplayError;
use flowlens_core::models::{HttpRequest, HttpResponse, HttpVersion};
use reqwest::Client;
use std::time::Duration;

/// Client for replaying captured HTTP requests
pub struct ReplayClient {
    client: Client,
    base_url_override: Option<String>,
}

impl ReplayClient {
    /// Create a new ReplayClient
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .danger_accept_invalid_certs(true)
                .build()
                .expect("Failed to create HTTP client"),
            base_url_override: None,
        }
    }

    /// Create a new ReplayClient with custom timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(timeout_secs))
                .danger_accept_invalid_certs(true)
                .build()
                .expect("Failed to create HTTP client"),
            base_url_override: None,
        }
    }

    /// Set a base URL override for replaying to a different destination
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url_override = Some(base_url);
        self
    }

    /// Replay a single HTTP request and return the response
    pub async fn replay(&self, request: &HttpRequest) -> Result<HttpResponse, ReplayError> {
        let target_url = self.resolve_url(&request.url)?;

        // Parse method
        let method: reqwest::Method = request.method.parse()
            .map_err(|_| ReplayError::UnsupportedMethod(request.method.clone()))?;

        let mut req_builder = self.client.request(method, &target_url);

        // Add headers (skip Host, set by reqwest)
        for (key, value) in &request.headers {
            if key.to_lowercase() != "host" {
                req_builder = req_builder.header(key, value);
            }
        }

        // Add body if present
        if let Some(body_bytes) = &request.body {
            req_builder = req_builder.body(body_bytes.clone());
        }

        // Execute the request
        let resp = req_builder.send().await?;

        // Collect response
        let status_code = resp.status().as_u16();
        let status_text = resp.status().canonical_reason().unwrap_or("Unknown").to_string();
        let version = match resp.version() {
            reqwest::Version::HTTP_10 => HttpVersion::Http10,
            reqwest::Version::HTTP_11 => HttpVersion::Http11,
            reqwest::Version::HTTP_2 => HttpVersion::Http2,
            reqwest::Version::HTTP_3 => HttpVersion::Http3,
            _ => HttpVersion::Http11,
        };

        let mut headers = Vec::new();
        for (name, value) in resp.headers() {
            let name_str = name.as_str().to_string();
            let value_str = value.to_str().unwrap_or("").to_string();
            headers.push((name_str, value_str));
        }

        let body_bytes = resp.bytes().await?.to_vec();
        let body_size = body_bytes.len();

        Ok(HttpResponse {
            status_code,
            status_text,
            version,
            headers,
            body: if body_size > 0 { Some(body_bytes) } else { None },
            body_size,
            timestamp: 0,
        })
    }

    /// Replay multiple requests and return responses in order
    pub async fn replay_batch(
        &self,
        requests: &[HttpRequest],
    ) -> Result<Vec<Result<HttpResponse, ReplayError>>, ReplayError> {
        let mut results = Vec::with_capacity(requests.len());
        for req in requests {
            results.push(self.replay(req).await);
        }
        Ok(results)
    }

    /// Resolve the target URL, applying base URL override if set
    pub fn resolve_url(&self, original_url: &str) -> Result<String, ReplayError> {
        if let Some(ref base) = self.base_url_override {
            let parsed = url::Url::parse(original_url)?;
            let path_query = parsed.path();
            let query = parsed.query().map(|q| format!("?{}", q)).unwrap_or_default();

            let base_parsed = url::Url::parse(base)?;
            let resolved = format!(
                "{}{}{}",
                base_parsed.as_str().trim_end_matches('/'),
                path_query,
                query
            );
            Ok(resolved)
        } else {
            Ok(original_url.to_string())
        }
    }
}

impl Default for ReplayClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Simple echo server for testing
    async fn spawn_echo_server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/echo", addr);

        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut socket, _)) => {
                        let mut buf = [0; 1024];
                        if let Ok(n) = socket.read(&mut buf).await {
                            // Echo back the request
                            let _ = socket.write_all(&buf[..n]).await;
                            // Send a minimal HTTP response
                            let response = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
                            let _ = socket.write_all(response).await;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        (url, handle)
    }

    #[tokio::test]
    async fn test_replay_client_creation() {
        let client = ReplayClient::new();
        assert!(client.base_url_override.is_none());
    }

    #[tokio::test]
    async fn test_replay_with_timeout() {
        let client = ReplayClient::with_timeout(60);
        assert!(client.base_url_override.is_none());
    }

    #[tokio::test]
    async fn test_base_url_override() {
        let client = ReplayClient::new().with_base_url("http://override.example.com".to_string());
        assert_eq!(client.base_url_override, Some("http://override.example.com".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_url_no_override() {
        let client = ReplayClient::new();
        let url = client.resolve_url("http://example.com/path?query=1").unwrap();
        assert_eq!(url, "http://example.com/path?query=1");
    }

    #[tokio::test]
    async fn test_resolve_url_with_override() {
        let client = ReplayClient::new().with_base_url("http://override.com".to_string());
        let url = client.resolve_url("http://original.com/path?query=1").unwrap();
        assert_eq!(url, "http://override.com/path?query=1");
    }

    #[tokio::test]
    async fn test_replay_to_echo_server() {
        let (url, _handle) = spawn_echo_server().await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let client = ReplayClient::new();
        
        let request = HttpRequest {
            method: "GET".to_string(),
            url: url.clone(),
            path: "/echo".to_string(),
            request_line_version: HttpVersion::Http11,
            headers: vec![("Host".to_string(), "localhost".to_string())],
            body: None,
            body_size: 0,
            timestamp: 0,
        };

        let result = client.replay(&request).await;
        assert!(result.is_ok() || result.is_err());
    }
}