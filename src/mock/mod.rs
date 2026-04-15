//! Mock/Rewrite rules engine — YAML-based request matching and response override.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchRule {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub query: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub body_contains: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MockResponse {
    #[serde(default = "default_type")]
    #[serde(rename = "type")]
    pub resp_type: String,
    #[serde(default = "default_status")]
    pub status: u16,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub delay_ms: u64,
}

fn default_type() -> String { "json".to_string() }
fn default_status() -> u16 { 200 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRuleData {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub match_: Option<MatchRule>,
    #[serde(rename = "match", default)]
    pub match_field: Option<MatchRule>,
    #[serde(default)]
    pub response: MockResponse,
}

fn default_true() -> bool { true }

#[derive(Deserialize)]
struct RulesFile {
    rules: Vec<MockRuleData>,
}

#[derive(Debug)]
pub struct MockRule {
    pub id: String,
    pub enabled: bool,
    pub priority: i32,
    pub match_rule: MatchRule,
    pub response: MockResponse,
    pub hit_count: u32,
}

pub struct MockEngine {
    pub rules: Vec<MockRule>,
    yaml_path: Option<PathBuf>,
    last_modified: u64,
}

impl MockEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            yaml_path: None,
            last_modified: 0,
        }
    }

    pub fn load(&mut self, path: &str) -> Result<String, String> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(format!("File not found: {}", path.display()));
        }

        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let data: RulesFile = serde_yaml::from_str(&content).map_err(|e| format!("YAML error: {e}"))?;

        self.rules.clear();
        for rule_data in data.rules {
            let match_rule = rule_data.match_field
                .or(rule_data.match_)
                .unwrap_or_default();

            self.rules.push(MockRule {
                id: rule_data.id,
                enabled: rule_data.enabled,
                priority: rule_data.priority,
                match_rule,
                response: rule_data.response,
                hit_count: 0,
            });
        }

        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mtime = path.metadata()
            .and_then(|m| m.modified())
            .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(|e| std::io::Error::other(e)))
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_modified = mtime;
        self.yaml_path = Some(path);

        Ok(format!("Loaded {} rules", self.rules.len()))
    }

    pub fn reload(&mut self) -> Result<String, String> {
        match &self.yaml_path {
            Some(p) => {
                let path = p.to_string_lossy().to_string();
                self.load(&path)
            }
            None => Err("No rules file loaded".to_string()),
        }
    }

    #[allow(dead_code)] // TODO: wire up to traffic proxy when mock feature lands
    pub fn check_hot_reload(&mut self) -> bool {
        let Some(ref path) = self.yaml_path else { return false };
        if !path.exists() { return false; }

        let mtime = path.metadata()
            .and_then(|m| m.modified())
            .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(|e| std::io::Error::other(e)))
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if mtime > self.last_modified {
            self.reload().is_ok()
        } else {
            false
        }
    }

    pub fn enable_rule(&mut self, id: &str) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = true;
            true
        } else {
            false
        }
    }

    pub fn disable_rule(&mut self, id: &str) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = false;
            true
        } else {
            false
        }
    }

    #[allow(dead_code)] // TODO: wire up to traffic proxy when mock feature lands
    pub fn get_rule(&self, id: &str) -> Option<&MockRule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// Match a request (method, path, host, query_params) against rules.
    /// Returns the response body and status if matched.
    #[allow(dead_code)] // TODO: wire up to traffic proxy when mock feature lands
    pub fn match_request(&mut self, method: &str, path: &str, host: &str) -> Option<(u16, String)> {
        for rule in &mut self.rules {
            if !rule.enabled {
                continue;
            }
            let m = &rule.match_rule;
            if !m.method.is_empty() && m.method.to_uppercase() != method.to_uppercase() {
                continue;
            }
            if !m.path.is_empty() && !path.contains(&m.path) {
                continue;
            }
            if !m.host.is_empty() && !host.to_lowercase().contains(&m.host.to_lowercase()) {
                continue;
            }
            rule.hit_count += 1;

            let body = if rule.response.resp_type == "file" && !rule.response.file.is_empty() {
                let file_path = if Path::new(&rule.response.file).is_absolute() {
                    PathBuf::from(&rule.response.file)
                } else if let Some(ref yaml_dir) = self.yaml_path {
                    yaml_dir.parent().unwrap_or(Path::new(".")).join(&rule.response.file)
                } else {
                    PathBuf::from(&rule.response.file)
                };
                fs::read_to_string(file_path).unwrap_or_else(|_| {
                    format!("{{\"error\": \"Mock file not found: {}\"}}", rule.response.file)
                })
            } else if rule.response.resp_type == "error" {
                format!("{{\"error\": \"Mocked error\", \"status\": {}}}", rule.response.status)
            } else if rule.response.resp_type == "empty" {
                String::new()
            } else if !rule.response.body.is_empty() {
                rule.response.body.clone()
            } else {
                "{}".to_string()
            };

            return Some((rule.response.status, body));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SAMPLE_YAML: &str = r#"
rules:
  - id: test_mock
    enabled: true
    priority: 10
    match:
      method: GET
      path: /api/test
    response:
      type: json
      status: 200
      body: '{"ok": true}'

  - id: disabled_mock
    enabled: false
    match:
      path: /api/other
    response:
      type: error
      status: 500
"#;

    fn write_yaml(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_load_rules() {
        let f = write_yaml(SAMPLE_YAML);
        let mut engine = MockEngine::new();
        let result = engine.load(f.path().to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(engine.rules.len(), 2);
    }

    #[test]
    fn test_priority_order() {
        let f = write_yaml(SAMPLE_YAML);
        let mut engine = MockEngine::new();
        engine.load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(engine.rules[0].id, "test_mock");
        assert_eq!(engine.rules[0].priority, 10);
    }

    #[test]
    fn test_enable_disable() {
        let f = write_yaml(SAMPLE_YAML);
        let mut engine = MockEngine::new();
        engine.load(f.path().to_str().unwrap()).unwrap();

        assert!(engine.disable_rule("test_mock"));
        assert!(!engine.get_rule("test_mock").unwrap().enabled);
        assert!(engine.enable_rule("test_mock"));
        assert!(engine.get_rule("test_mock").unwrap().enabled);
        assert!(!engine.enable_rule("nonexistent"));
    }

    #[test]
    fn test_match_request() {
        let f = write_yaml(SAMPLE_YAML);
        let mut engine = MockEngine::new();
        engine.load(f.path().to_str().unwrap()).unwrap();

        let result = engine.match_request("GET", "/api/test", "example.com");
        assert!(result.is_some());
        let (status, body) = result.unwrap();
        assert_eq!(status, 200);
        assert!(body.contains("ok"));
    }

    #[test]
    fn test_disabled_not_matched() {
        let f = write_yaml(SAMPLE_YAML);
        let mut engine = MockEngine::new();
        engine.load(f.path().to_str().unwrap()).unwrap();

        let result = engine.match_request("GET", "/api/other", "example.com");
        assert!(result.is_none()); // disabled
    }

    #[test]
    fn test_reload() {
        let f = write_yaml(SAMPLE_YAML);
        let mut engine = MockEngine::new();
        engine.load(f.path().to_str().unwrap()).unwrap();
        assert!(engine.reload().is_ok());
    }

    #[test]
    fn test_missing_file() {
        let mut engine = MockEngine::new();
        assert!(engine.load("/nonexistent/path.yaml").is_err());
    }

    #[test]
    fn test_invalid_yaml() {
        let f = write_yaml("not: a: valid: yaml: [");
        let mut engine = MockEngine::new();
        assert!(engine.load(f.path().to_str().unwrap()).is_err());
    }
}
