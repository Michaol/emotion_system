use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::EmotionState;

/// 其他 Agent 的情绪摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub agent_id: String,
    pub v: f32,
    pub a: f32,
    pub d: f32,
    pub last_updated_ms: i64,
}

/// 扫描指定目录下的 Agent 状态文件
///
/// 约定：每个 Agent 的状态文件命名为 `<agent_id>_state.json`
pub fn scan_other_agents(scan_dir: &str, exclude_self: &str) -> Vec<AgentSummary> {
    let dir = Path::new(scan_dir);
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut results = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.ends_with("_state.json") || path.to_str() == Some(exclude_self) {
                    continue;
                }

                let agent_id = name.trim_end_matches("_state.json").to_string();

                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(state) = serde_json::from_str::<EmotionState>(&content) {
                        results.push(AgentSummary {
                            agent_id,
                            v: state.current.v,
                            a: state.current.a,
                            d: state.current.d,
                            last_updated_ms: state.last_updated_ms,
                        });
                    }
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let results = scan_other_agents(dir.path().to_str().unwrap(), "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_nonexistent_dir() {
        let results = scan_other_agents("/nonexistent/path", "");
        assert!(results.is_empty());
    }
}
