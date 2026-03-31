//! 昼夜衰减配置与时间判断

use serde::{Deserialize, Serialize};

/// 昼夜衰减配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecaySchedule {
    /// 日间开始小时 (默认 8)
    #[serde(default = "default_day_start")]
    pub day_start_hour: u8,
    /// 夜间开始小时 (默认 0)
    #[serde(default = "default_night_start")]
    pub night_start_hour: u8,
    /// 夜间 Valence 衰减倍率 (默认 1.5)
    #[serde(default = "default_night_v")]
    pub night_v_multiplier: f32,
    /// 夜间 Arousal 衰减倍率 (默认 3.0)
    #[serde(default = "default_night_a")]
    pub night_a_multiplier: f32,
    /// 夜间 Dominance 衰减倍率 (默认 0.5)
    #[serde(default = "default_night_d")]
    pub night_d_multiplier: f32,
    /// 时区偏移 (小时, 默认 +8 = UTC+8)
    #[serde(default = "default_timezone")]
    pub timezone_offset: i8,
}

fn default_day_start() -> u8 {
    8
}
fn default_night_start() -> u8 {
    0
}
fn default_night_v() -> f32 {
    1.5
}
fn default_night_a() -> f32 {
    3.0
}
fn default_night_d() -> f32 {
    0.5
}
fn default_timezone() -> i8 {
    8
}

impl Default for DecaySchedule {
    fn default() -> Self {
        Self {
            day_start_hour: 8,
            night_start_hour: 0,
            night_v_multiplier: 1.5,
            night_a_multiplier: 3.0,
            night_d_multiplier: 0.5,
            timezone_offset: 8,
        }
    }
}

impl DecaySchedule {
    /// 根据时间戳(ms)判断是否为夜间
    ///
    /// 支持跨午夜配置 (如 night_start=22, day_start=6)。
    #[must_use]
    pub fn is_night(&self, timestamp_ms: i64) -> bool {
        let hour = self.local_hour(timestamp_ms);
        if self.night_start_hour < self.day_start_hour {
            // 正常: night 0..day_start (如 0..8)
            hour < self.day_start_hour
        } else {
            // 跨午夜: night [night_start..24) ∪ [0..day_start)
            hour >= self.night_start_hour || hour < self.day_start_hour
        }
    }

    /// 返回本地小时 (0-23)
    #[must_use]
    pub fn local_hour(&self, timestamp_ms: i64) -> u8 {
        let utc_hours = (timestamp_ms / 3_600_000) % 24;
        let local = (utc_hours + i64::from(self.timezone_offset)).rem_euclid(24);
        local as u8
    }

    /// 返回时间阶段标签
    #[must_use]
    pub fn time_phase_label(&self, timestamp_ms: i64) -> &'static str {
        if self.is_night(timestamp_ms) {
            "sleeping"
        } else {
            "daytime"
        }
    }

    /// 获取指定维度的当前衰减倍率
    #[must_use]
    pub fn multiplier_for(&self, dimension: &str, timestamp_ms: i64) -> f32 {
        if !self.is_night(timestamp_ms) {
            return 1.0;
        }
        match dimension {
            "v" | "valence" => self.night_v_multiplier,
            "a" | "arousal" => self.night_a_multiplier,
            "d" | "dominance" => self.night_d_multiplier,
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-03-21 23:00 UTC = 2026-03-22 07:00 UTC+8
    const LATE_UTC_MS: i64 = 1_774_124_400_000;
    // 2026-03-21 04:00 UTC = 2026-03-21 12:00 UTC+8
    const MIDDAY_UTC_MS: i64 = 1_774_065_600_000;

    #[test]
    fn test_default_schedule() {
        let s = DecaySchedule::default();
        assert_eq!(s.day_start_hour, 8);
        assert_eq!(s.night_start_hour, 0);
        assert_eq!(s.timezone_offset, 8);
    }

    #[test]
    fn test_local_hour_utc8() {
        let s = DecaySchedule::default();
        // UTC 04:00 = UTC+8 12:00
        let ts = 1_774_065_600_000; // 2026-03-21 04:00 UTC
        assert_eq!(s.local_hour(ts), 12);
    }

    #[test]
    fn test_is_night_3am_local() {
        let s = DecaySchedule::default();
        // UTC 19:00 2026-03-20 = UTC+8 03:00 2026-03-21
        let ts = 1_774_033_200_000; // 2026-03-20 19:00 UTC = 03:00 local
        assert!(s.is_night(ts), "03:00 local should be night");
    }

    #[test]
    fn test_is_day_noon_local() {
        let s = DecaySchedule::default();
        assert!(!s.is_night(MIDDAY_UTC_MS), "12:00 local should be day");
    }

    #[test]
    fn test_is_night_7am_local() {
        let s = DecaySchedule::default();
        // UTC 23:00 = UTC+8 07:00 — still night (before 08:00)
        assert!(s.is_night(LATE_UTC_MS), "07:00 local should be night");
    }

    #[test]
    fn test_is_day_8am_local() {
        let s = DecaySchedule::default();
        // UTC 00:00 = UTC+8 08:00 — day starts
        let ts = 1_774_051_200_000; // 2026-03-21 00:00 UTC = 08:00 local
        assert!(!s.is_night(ts), "08:00 local should be day");
    }

    #[test]
    fn test_multiplier_for_arousal_night() {
        let s = DecaySchedule::default();
        let ts = 1_774_033_200_000; // 03:00 local
        assert!((s.multiplier_for("arousal", ts) - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_multiplier_for_arousal_day() {
        let s = DecaySchedule::default();
        assert!((s.multiplier_for("arousal", MIDDAY_UTC_MS) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_multiplier_for_valence_night() {
        let s = DecaySchedule::default();
        let ts = 1_774_033_200_000;
        assert!((s.multiplier_for("valence", ts) - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_multiplier_for_dominance_night() {
        let s = DecaySchedule::default();
        let ts = 1_774_033_200_000;
        assert!((s.multiplier_for("dominance", ts) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_time_phase_label_sleeping() {
        let s = DecaySchedule::default();
        let ts = 1_774_033_200_000; // 03:00 local
        assert_eq!(s.time_phase_label(ts), "sleeping");
    }

    #[test]
    fn test_time_phase_label_daytime() {
        let s = DecaySchedule::default();
        assert_eq!(s.time_phase_label(MIDDAY_UTC_MS), "daytime");
    }

    #[test]
    fn test_is_night_cross_midnight_config() {
        // 跨午夜配置: night 22:00-06:00
        let s = DecaySchedule {
            night_start_hour: 22,
            day_start_hour: 6,
            ..DecaySchedule::default()
        };
        // UTC 16:00 = 00:00 local → should be night (>= 22)
        let ts_night = 1_774_101_600_000;
        assert!(s.is_night(ts_night), "00:00 local should be night");

        // UTC 00:00 = 08:00 local → should be day
        let ts_day = 1_774_051_200_000;
        assert!(!s.is_night(ts_day), "08:00 local should be day");

        // UTC 20:00 = 04:00 local → should be night (before 06:00)
        let ts_early = 1_774_116_000_000;
        assert!(s.is_night(ts_early), "04:00 local should be night");
    }
}
