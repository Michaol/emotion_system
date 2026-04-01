//! 按需衰减模块：仅在读取时计算，无后台定时器
//!
//! 核心公式: val(t) = baseline + (val(t₀) - baseline) × e^(-rate × Δt_hours)

/// 计算衰减后的值
#[must_use]
pub fn apply_decay(current: f32, baseline: f32, rate: f32, delta_hours: f64) -> f32 {
    let factor = (-f64::from(rate) * delta_hours).exp() as f32;
    baseline + (current - baseline) * factor
}

/// 半衰期 (小时) → 衰减率
#[must_use]
pub fn half_life_to_rate(half_life_hours: f32) -> f32 {
    2.0_f32.ln() / half_life_hours
}

/// 衰减率 → 半衰期 (小时)
#[must_use]
pub fn rate_to_half_life(rate: f32) -> f32 {
    2.0_f32.ln() / rate
}

/// 毫秒时间戳差 → 小时
#[must_use]
pub fn ms_to_hours(delta_ms: i64) -> f64 {
    delta_ms as f64 / 3_600_000.0
}

/// 昼夜分段衰减
///
/// 当 last_ms → now_ms 跨越昼夜边界时，逐小时分段计算：
/// - 白天段用 rate × 1.0
/// - 夜间段用 rate × night_multiplier
///
/// 性能优化：连续同相段合并为单次衰减调用。
#[must_use]
pub fn split_decay(
    current: f32,
    baseline: f32,
    rate: f32,
    last_ms: i64,
    now_ms: i64,
    dimension: &str,
    schedule: &crate::decay_schedule::DecaySchedule,
) -> f32 {
    if now_ms <= last_ms {
        return current;
    }

    let total_hours = ms_to_hours(now_ms - last_ms);

    // 短时间差 (< 1h) 直接用单段衰减
    if total_hours < 1.0 {
        let mult = schedule.multiplier_for(dimension, last_ms);
        return apply_decay(current, baseline, rate * mult, total_hours);
    }

    // 合并连续同相段：记录 (hours, multiplier) 片段
    let mut val = current;
    let mut cursor = last_ms;
    let step_ms: i64 = 3_600_000;

    while cursor < now_ms {
        // 当前段的相位和倍率
        let phase_mult = schedule.multiplier_for(dimension, cursor);

        // 扫描连续同相段，合并为一个大块
        let mut segment_ms: i64 = 0;
        let mut probe = cursor;
        while probe < now_ms {
            let chunk = if (now_ms - probe) < step_ms {
                now_ms - probe
            } else {
                step_ms
            };
            // 用段开始时间判断倍率（同相段内倍率不变）
            if (schedule.multiplier_for(dimension, probe) - phase_mult).abs() < 0.001 {
                segment_ms += chunk;
                probe += chunk;
            } else {
                break;
            }
        }

        let segment_hours = ms_to_hours(segment_ms);
        val = apply_decay(val, baseline, rate * phase_mult, segment_hours);
        cursor += segment_ms;
    }

    val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decay_at_zero_time() {
        let result = apply_decay(0.8, 0.0, 0.058, 0.0);
        assert!((result - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_decay_approaches_baseline() {
        // 经过大量时间后应趋近 baseline
        let result = apply_decay(0.8, 0.0, 0.058, 1000.0);
        assert!(result.abs() < 0.01);
    }

    #[test]
    fn test_half_life_conversion() {
        let rate = half_life_to_rate(24.0);
        let hl = rate_to_half_life(rate);
        assert!((hl - 24.0).abs() < 1e-4);
    }

    #[test]
    fn test_decay_at_half_life() {
        let half_life = 12.0_f32;
        let rate = half_life_to_rate(half_life);
        // 经过一个半衰期后，差距应减半
        let result = apply_decay(1.0, 0.0, rate, f64::from(half_life));
        assert!((result - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_ms_to_hours() {
        assert!((ms_to_hours(3_600_000) - 1.0).abs() < 1e-9);
        assert!((ms_to_hours(7_200_000) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_split_decay_pure_daytime() {
        // 12:00-14:00 UTC = 20:00-22:00 UTC+8 (daytime)
        let schedule = crate::decay_schedule::DecaySchedule::default();
        let last_ms = 1_774_084_800_000; // 12:00 UTC = 20:00 local
        let now_ms = last_ms + 7_200_000; // +2h
        let result = split_decay(0.8, 0.0, 0.058, last_ms, now_ms, "v", &schedule);
        let expected = apply_decay(0.8, 0.0, 0.058, 2.0);
        assert!(
            (result - expected).abs() < 0.001,
            "Daytime split should match single decay"
        );
    }

    #[test]
    fn test_split_decay_pure_nighttime_arousal() {
        // 03:00-06:00 local (night) — Arousal should decay 3x faster
        let schedule = crate::decay_schedule::DecaySchedule::default();
        // UTC 19:00 2026-03-20 = 03:00 local 2026-03-21
        let last_ms = 1_774_033_200_000;
        let now_ms = last_ms + 10_800_000; // +3h
        let result = split_decay(0.8, 0.0, 0.058, last_ms, now_ms, "arousal", &schedule);
        let expected = apply_decay(0.8, 0.0, 0.058 * 3.0, 3.0);
        assert!(
            (result - expected).abs() < 0.001,
            "Night arousal should decay 3x"
        );
    }

    #[test]
    fn test_split_decay_crosses_midnight() {
        let schedule = crate::decay_schedule::DecaySchedule::default();
        let last_ms = 1_774_092_000_000; // UTC 14:00 = 22:00 local
        let now_ms = last_ms + 43_200_000; // +12h

        // Debug: check each hour's phase
        for h in 0..12 {
            let ts = last_ms + h * 3_600_000;
            let phase = schedule.time_phase_label(ts);
            let mult = schedule.multiplier_for("arousal", ts);
            let local_h = schedule.local_hour(ts);
            println!("  chunk {h}: local_hour={local_h}, phase={phase}, mult={mult}");
        }

        let result = split_decay(0.8, 0.0, 0.058, last_ms, now_ms, "arousal", &schedule);
        println!("  result={result}");

        // Manual calculation
        let mut expected = 0.8_f64;
        let rate = 0.058_f64;
        for h in 0..12 {
            let ts = last_ms + h * 3_600_000;
            let mult = schedule.multiplier_for("arousal", ts);
            expected = 0.0 + (expected - 0.0) * (-rate * f64::from(mult)).exp();
        }
        println!("  expected={expected}");
        assert!(
            (result as f64 - expected).abs() < 0.001,
            "got {result}, expected {expected}"
        );
    }

    #[test]
    fn test_split_decay_dominance_night_slow() {
        // Dominance should decay slower at night (×0.5)
        let schedule = crate::decay_schedule::DecaySchedule::default();
        let last_ms = 1_774_033_200_000; // 03:00 local
        let now_ms = last_ms + 10_800_000; // +3h
        let result = split_decay(0.8, 0.0, 0.058, last_ms, now_ms, "dominance", &schedule);
        let expected = apply_decay(0.8, 0.0, 0.058 * 0.5, 3.0);
        assert!(
            (result - expected).abs() < 0.001,
            "Night dominance should decay 0.5x"
        );
    }
}
