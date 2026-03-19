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
        let rate = half_life_to_rate(12.0);
        let hl = rate_to_half_life(rate);
        assert!((hl - 12.0).abs() < 1e-4);
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
}
