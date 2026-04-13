//! Traffic proxy — HTTP/HTTPS interception via built-in async proxy.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct TrafficEntry {
    pub id: usize,
    pub timestamp: String,
    pub method: String,
    pub url: String,
    pub host: String,
    pub path: String,
    pub status: Option<u16>,
    pub request_headers: HashMap<String, String>,
    pub request_body: Vec<u8>,
    pub response_headers: HashMap<String, String>,
    pub response_body: Vec<u8>,
}

#[derive(Default)]
pub struct TrafficFilter {
    pub host: String,
    pub path: String,
    pub method: String,
    pub status: Option<u16>,
    pub body_search: String,
}

impl TrafficFilter {
    pub fn matches(&self, entry: &TrafficEntry) -> bool {
        if !self.host.is_empty() && !entry.host.to_lowercase().contains(&self.host.to_lowercase()) {
            return false;
        }
        if !self.path.is_empty() && !entry.path.to_lowercase().contains(&self.path.to_lowercase()) {
            return false;
        }
        if !self.method.is_empty() && entry.method.to_uppercase() != self.method.to_uppercase() {
            return false;
        }
        if let Some(status) = self.status {
            if entry.status != Some(status) {
                return false;
            }
        }
        if !self.body_search.is_empty() {
            let needle = self.body_search.to_lowercase();
            let req_body = String::from_utf8_lossy(&entry.request_body).to_lowercase();
            let resp_body = String::from_utf8_lossy(&entry.response_body).to_lowercase();
            if !req_body.contains(&needle) && !resp_body.contains(&needle) {
                return false;
            }
        }
        true
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Shared traffic state accessible from proxy threads and main shell.
pub struct TrafficState {
    pub entries: Vec<TrafficEntry>,
    pub filter: TrafficFilter,
    counter: usize,
}

impl Default for TrafficState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            filter: TrafficFilter::default(),
            counter: 0,
        }
    }
}

impl TrafficState {
    pub fn add_request(&mut self, method: &str, url: &str, host: &str, path: &str, headers: HashMap<String, String>, body: Vec<u8>) -> usize {
        self.counter += 1;
        let id = self.counter;
        self.entries.push(TrafficEntry {
            id,
            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            method: method.to_string(),
            url: url.to_string(),
            host: host.to_string(),
            path: path.to_string(),
            status: None,
            request_headers: headers,
            request_body: body,
            response_headers: HashMap::new(),
            response_body: Vec::new(),
        });
        id
    }

    pub fn set_response(&mut self, id: usize, status: u16, headers: HashMap<String, String>, body: Vec<u8>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.status = Some(status);
            entry.response_headers = headers;
            entry.response_body = body;
        }
    }

    pub fn get_filtered(&self, limit: usize) -> Vec<&TrafficEntry> {
        self.entries
            .iter()
            .filter(|e| self.filter.matches(e))
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn get_entry(&self, id: usize) -> Option<&TrafficEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.counter = 0;
    }
}

/// Thread-safe wrapper for traffic state.
pub type SharedTrafficState = Arc<Mutex<TrafficState>>;

pub fn new_shared_state() -> SharedTrafficState {
    Arc::new(Mutex::new(TrafficState::default()))
}

/// Proxy server placeholder — uses mitmproxy as external process.
/// For full Rust-native proxy, `hyper` + `rcgen` + `tokio-rustls` can be used.
pub struct TrafficProxy {
    pub state: SharedTrafficState,
    pub listen_port: u16,
    running: bool,
    child: Option<tokio::process::Child>,
}

impl TrafficProxy {
    pub fn new(port: u16) -> Self {
        Self {
            state: new_shared_state(),
            listen_port: port,
            running: false,
            child: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub async fn start(&mut self) -> Result<String, String> {
        if self.running {
            return Err("Proxy already running".to_string());
        }

        // Try to start mitmproxy as external process (mitmdump)
        let child = tokio::process::Command::new("mitmdump")
            .args(["--listen-port", &self.listen_port.to_string(), "--set", "stream_large_bodies=1", "-q"])
            .kill_on_drop(true)
            .spawn();

        match child {
            Ok(c) => {
                self.child = Some(c);
                self.running = true;
                Ok(format!("Proxy started on port {}", self.listen_port))
            }
            Err(_) => {
                Err("mitmdump not found. Install mitmproxy: brew install mitmproxy".to_string())
            }
        }
    }

    pub async fn stop(&mut self) -> Result<String, String> {
        if !self.running {
            return Err("Proxy not running".to_string());
        }
        if let Some(ref mut child) = self.child {
            let _ = child.kill().await;
        }
        self.child = None;
        self.running = false;
        Ok("Proxy stopped".to_string())
    }
}
