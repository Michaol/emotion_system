use std::path::Path;

use crate::config::ConfigError;
use crate::engine::Engine;

/// 保存状态到文件 (原子写入: 先写 .tmp，再 rename)
pub fn save_state(engine: &mut Engine) -> Result<(), ConfigError> {
    // BUG-L4: 自动创建父目录
    if let Some(parent) = Path::new(&engine.state_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&engine.state)?;
    let tmp_path = format!("{}.tmp", engine.state_path);
    std::fs::write(&tmp_path, &json)?;
    std::fs::rename(&tmp_path, &engine.state_path)?;
    engine.dirty = false;
    Ok(())
}

/// 从文件加载状态
pub fn load_state(engine: &mut Engine) -> Result<(), ConfigError> {
    let path = Path::new(&engine.state_path);
    let tmp_path_str = format!("{}.tmp", engine.state_path);
    let tmp_path = Path::new(&tmp_path_str);

    // 崩溃恢复：如果主文件不存在但 tmp 存在，从 tmp 恢复
    if !path.exists() && tmp_path.exists() {
        std::fs::rename(tmp_path, path)?;
    }

    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        engine.state = serde_json::from_str(&content)?;
        engine.dirty = false;
    }
    Ok(())
}

/// 仅在脏且距上次保存超过最小间隔时保存
pub fn maybe_save(engine: &mut Engine, last_save_ms: &mut i64, min_interval_ms: i64) -> Result<bool, ConfigError> {
    if !engine.dirty {
        return Ok(false);
    }
    let now = crate::engine::now_ms();
    if now - *last_save_ms < min_interval_ms {
        return Ok(false);
    }
    save_state(engine)?;
    *last_save_ms = now;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let events = r#"{"version":"1.0","events":{"joy":{"delta_v":0.4,"delta_a":0.2,"delta_d":0.1}}}"#;
        let behaviors = r#"{"version":"1.0","behaviors":[],"default":{"tone":"neutral","speed":"moderate","description":"stable"}}"#;
        std::fs::write(dir.path().join("default_events.json"), events).unwrap();
        std::fs::write(dir.path().join("default_behavior.json"), behaviors).unwrap();

        let state_path = dir.path().join("test_state.json");
        let state_str = state_path.to_str().unwrap();
        let config_str = dir.path().to_str().unwrap();

        let mut engine = Engine::new(config_str, state_str, None).unwrap();
        engine.apply_event("joy", 1.0).unwrap();

        save_state(&mut engine).unwrap();
        assert!(!engine.dirty);
        assert!(state_path.exists());

        // 修改状态后重新加载
        engine.state.current.v = -0.99;
        load_state(&mut engine).unwrap();
        assert!(engine.state.current.v > 0.0); // 恢复了保存前的值
    }
}
