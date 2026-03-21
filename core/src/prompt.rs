use crate::engine::{now_ms, Engine};

/// 将当前情绪状态格式化为 `<emotion_state>` XML 块
pub fn format_emotion_prompt(engine: &mut Engine) -> String {
    let now = now_ms();
    let snap = engine.get_state();
    let state = &engine.state;

    let mut xml = String::from("<emotion_state>\n");

    // dimensions: 仅列出偏离 baseline > 0.15 的维度
    xml.push_str("  <dimensions>\n    ");
    let mut dims = Vec::new();
    let dv = state.current.v - state.baseline.v;
    if dv.abs() > 0.15 {
        let dir = if dv > 0.0 { "elevated" } else { "lowered" };
        dims.push(format!("valence: {dir} ({:.2})", state.current.v));
    }
    let da = state.current.a - state.baseline.a;
    if da.abs() > 0.15 {
        let dir = if da > 0.0 { "elevated" } else { "lowered" };
        dims.push(format!("arousal: {dir} ({:.2})", state.current.a));
    }
    let dd = state.current.d - state.baseline.d;
    if dd.abs() > 0.15 {
        let dir = if dd > 0.0 { "elevated" } else { "lowered" };
        dims.push(format!("dominance: {dir} ({:.2})", state.current.d));
    }
    if dims.is_empty() {
        dims.push("all dimensions near baseline".to_string());
    }
    xml.push_str(&dims.join(", "));
    xml.push_str("\n  </dimensions>\n");

    // tone
    xml.push_str(&format!("  <tone>{}</tone>\n", snap.tone));

    // time_phase (daytime/sleeping)
    let phase = state.decay_schedule.time_phase_label(now);
    xml.push_str(&format!("  <time_phase>{phase}</time_phase>\n"));

    // plutchik
    xml.push_str(&format!(
        "  <plutchik label=\"{}\" confidence=\"{:.2}\"/>\n",
        snap.plutchik.label, snap.plutchik.confidence
    ));

    // rumination
    if !state.ruminations.is_empty() {
        xml.push_str(&format!(
            "  <rumination active=\"{}\">\n",
            state.ruminations.len()
        ));
        for r in &state.ruminations {
            xml.push_str(&format!(
                "    Processing residual {} ({} rounds remaining)\n",
                r.source_event, r.remaining_rounds
            ));
        }
        xml.push_str("  </rumination>\n");
    }

    // memories (最近 3 条按有效强度排序)
    if !state.memories.is_empty() {
        let top_memories = crate::memory::retrieve_memories(&state.memories, &state.current, 3);
        xml.push_str(&format!("  <memories recent=\"{}\">\n", top_memories.len()));
        for m in &top_memories {
            let ts = format_timestamp(m.timestamp_ms);
            xml.push_str(&format!(
                "    {}: {} (salience: {:.2}, strength: {:.2})\n",
                ts,
                m.event_name,
                m.salience,
                m.effective_strength()
            ));
        }
        xml.push_str("  </memories>\n");
    }

    // stimuli (保留向后兼容，最近 3 条)
    let recent: Vec<_> = state.stimuli_history.iter().rev().take(3).collect();
    if !recent.is_empty() {
        xml.push_str(&format!("  <stimuli recent=\"{}\">\n", recent.len()));
        for s in &recent {
            let ts = format_timestamp(s.timestamp_ms);
            xml.push_str(&format!(
                "    {}: Experienced {} (V:{:+.2}, A:{:+.2}, D:{:+.2})\n",
                ts, s.event_name, s.delta.v, s.delta.a, s.delta.d
            ));
        }
        xml.push_str("  </stimuli>\n");
    }

    xml.push_str("</emotion_state>");
    xml
}

/// 简单的时间戳格式化 (UTC)
fn format_timestamp(ms: i64) -> String {
    let secs = ms / 1000;
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    format!("{hours:02}:{mins:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_contains_xml_tags() {
        let dir = tempfile::tempdir().unwrap();
        let events =
            r#"{"version":"1.0","events":{"joy":{"delta_v":0.4,"delta_a":0.2,"delta_d":0.1}}}"#;
        let behaviors = r#"{"version":"1.0","behaviors":[],"default":{"tone":"neutral","speed":"moderate","description":"stable"}}"#;
        std::fs::write(dir.path().join("default_events.json"), events).unwrap();
        std::fs::write(dir.path().join("default_behavior.json"), behaviors).unwrap();

        let state_path = dir.path().join("test.json");
        let mut engine = Engine::new(
            dir.path().to_str().unwrap(),
            state_path.to_str().unwrap(),
            None,
        )
        .unwrap();

        engine.apply_event("joy", 1.0).unwrap();
        let xml = format_emotion_prompt(&mut engine);

        assert!(xml.starts_with("<emotion_state>"));
        assert!(xml.ends_with("</emotion_state>"));
        assert!(xml.contains("<dimensions>"));
        assert!(xml.contains("<tone>"));
        assert!(xml.contains("<plutchik"));
        assert!(xml.contains("<stimuli"));
    }

    #[test]
    fn test_format_includes_plutchik_label() {
        let dir = tempfile::tempdir().unwrap();
        let events =
            r#"{"version":"1.0","events":{"joy":{"delta_v":0.4,"delta_a":0.2,"delta_d":0.1}}}"#;
        let behaviors = r#"{"version":"1.0","behaviors":[],"default":{"tone":"neutral","speed":"moderate","description":"stable"}}"#;
        std::fs::write(dir.path().join("default_events.json"), events).unwrap();
        std::fs::write(dir.path().join("default_behavior.json"), behaviors).unwrap();

        let state_path = dir.path().join("test.json");
        let mut engine = Engine::new(
            dir.path().to_str().unwrap(),
            state_path.to_str().unwrap(),
            None,
        )
        .unwrap();

        engine.apply_event("joy", 1.0).unwrap();
        let xml = format_emotion_prompt(&mut engine);

        assert!(xml.contains("label="));
        assert!(xml.contains("confidence="));
    }

    #[test]
    fn test_format_includes_time_phase() {
        let dir = tempfile::tempdir().unwrap();
        let events =
            r#"{"version":"1.0","events":{"joy":{"delta_v":0.4,"delta_a":0.2,"delta_d":0.1}}}"#;
        let behaviors = r#"{"version":"1.0","behaviors":[],"default":{"tone":"neutral","speed":"moderate","description":"stable"}}"#;
        std::fs::write(dir.path().join("default_events.json"), events).unwrap();
        std::fs::write(dir.path().join("default_behavior.json"), behaviors).unwrap();

        let state_path = dir.path().join("test.json");
        let mut engine = Engine::new(
            dir.path().to_str().unwrap(),
            state_path.to_str().unwrap(),
            None,
        )
        .unwrap();

        engine.apply_event("joy", 1.0).unwrap();
        let xml = format_emotion_prompt(&mut engine);

        assert!(xml.contains("<time_phase>"));
        assert!(
            xml.contains("<time_phase>daytime</time_phase>")
                || xml.contains("<time_phase>sleeping</time_phase>")
        );
    }
}
