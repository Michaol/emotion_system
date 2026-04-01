use serde::{Deserialize, Serialize};

use crate::decay::half_life_to_rate;

/// OCEAN 人格五大特质
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OceanProfile {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
}

/// 检查人格是否全部为默认值 (0.5)
///
/// 用于判断 state 文件是由 EmotionState::default() 创建且从未加载过 config。
/// 浮点容差 ±0.001。
#[must_use]
pub fn is_default_personality(p: &OceanProfile) -> bool {
    const EPS: f32 = 0.001;
    (p.openness - 0.5).abs() < EPS
        && (p.conscientiousness - 0.5).abs() < EPS
        && (p.extraversion - 0.5).abs() < EPS
        && (p.agreeableness - 0.5).abs() < EPS
        && (p.neuroticism - 0.5).abs() < EPS
}

impl OceanProfile {
    /// 创建并 clamp 到 [0.0, 1.0]
    #[must_use]
    pub fn new(o: f32, c: f32, e: f32, a: f32, n: f32) -> Self {
        Self {
            openness: o.clamp(0.0, 1.0),
            conscientiousness: c.clamp(0.0, 1.0),
            extraversion: e.clamp(0.0, 1.0),
            agreeableness: a.clamp(0.0, 1.0),
            neuroticism: n.clamp(0.0, 1.0),
        }
    }

    /// 设置单个特质
    pub fn set_trait(&mut self, name: &str, value: f32) -> Result<(), String> {
        let v = value.clamp(0.0, 1.0);
        match name {
            "openness" => self.openness = v,
            "conscientiousness" => self.conscientiousness = v,
            "extraversion" => self.extraversion = v,
            "agreeableness" => self.agreeableness = v,
            "neuroticism" => self.neuroticism = v,
            _ => return Err(format!("Unknown personality trait: {name}")),
        }
        Ok(())
    }
}

impl Default for OceanProfile {
    fn default() -> Self {
        Self::new(0.5, 0.5, 0.5, 0.5, 0.5)
    }
}

/// 基于人格的 V/A/D 各维度衰减率
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayRates {
    pub v_rate: f32,
    pub a_rate: f32,
    pub d_rate: f32,
}

impl DecayRates {
    /// 基于默认半衰期 (V=24h, A=4h, D=36h) 和人格特质调制
    ///
    /// 半衰期依据: Verduyn & Lavrijsen (2014), Motivation and Emotion.
    /// - Valence: 24h (悲伤可持续数天，快乐数小时)
    /// - Arousal: 4h (生理激活消退快，恐惧 ~30min)
    /// - Dominance: 36h (支配感与自我认知相关，变化最慢)
    #[must_use]
    pub fn from_personality(p: &OceanProfile) -> Self {
        let base_v = half_life_to_rate(24.0);
        let base_a = half_life_to_rate(4.0);
        let base_d = half_life_to_rate(36.0);

        // 神经质↑ → 负面衰减更慢 (rate × 0.84~1.0)
        let n_mod = 1.0 - 0.16 * (p.neuroticism - 0.5).max(0.0);
        // 外向↑ → 悲伤/负面消退更快 (rate × 1.0~1.16)
        let e_mod = 1.0 + 0.16 * (p.extraversion - 0.5).max(0.0);
        // 开放↑ → 好奇/惊叹持续更久 (rate × 0.90~1.0)
        let o_mod = 1.0 - 0.10 * (p.openness - 0.5).max(0.0);

        Self {
            v_rate: base_v * n_mod * e_mod,
            a_rate: base_a * o_mod,
            d_rate: base_d,
        }
    }
}

impl Default for DecayRates {
    fn default() -> Self {
        Self::from_personality(&OceanProfile::default())
    }
}

/// 基于人格计算 V/A/D baseline 偏移
#[must_use]
pub fn compute_baseline(p: &OceanProfile) -> crate::vad::VadState {
    let v = 0.1 * (p.extraversion - 0.5) + 0.1 * (p.agreeableness - 0.5)
        - 0.15 * (p.neuroticism - 0.5)
        + 0.05 * (p.openness - 0.5);

    let a = 0.15 * (p.extraversion - 0.5) + 0.1 * (p.neuroticism - 0.5) + 0.05 * (p.openness - 0.5)
        - 0.05 * (p.conscientiousness - 0.5);

    let d = 0.05 * (p.extraversion - 0.5)
        - 0.05 * (p.agreeableness - 0.5)
        - 0.1 * (p.neuroticism - 0.5)
        + 0.1 * (p.conscientiousness - 0.5);

    crate::vad::VadState::new(v, a, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_profile() {
        let p = OceanProfile::default();
        assert_eq!(p.openness, 0.5);
        assert_eq!(p.neuroticism, 0.5);
    }

    #[test]
    fn test_set_trait() {
        let mut p = OceanProfile::default();
        p.set_trait("neuroticism", 0.8).unwrap();
        assert_eq!(p.neuroticism, 0.8);
    }

    #[test]
    fn test_set_trait_unknown() {
        let mut p = OceanProfile::default();
        assert!(p.set_trait("unknown", 0.5).is_err());
    }

    #[test]
    fn test_set_trait_clamps() {
        let mut p = OceanProfile::default();
        p.set_trait("openness", 1.5).unwrap();
        assert_eq!(p.openness, 1.0);
    }

    #[test]
    fn test_baseline_neutral_personality() {
        let p = OceanProfile::default();
        let b = compute_baseline(&p);
        // 中性人格 baseline 应接近 0
        assert!(b.v.abs() < 0.01);
        assert!(b.a.abs() < 0.01);
        assert!(b.d.abs() < 0.01);
    }

    #[test]
    fn test_high_neuroticism_slower_decay() {
        let low_n = OceanProfile::new(0.5, 0.5, 0.5, 0.5, 0.2);
        let high_n = OceanProfile::new(0.5, 0.5, 0.5, 0.5, 0.9);
        let dr_low = DecayRates::from_personality(&low_n);
        let dr_high = DecayRates::from_personality(&high_n);
        // 高神经质 → v_rate 更低 (衰减更慢)
        assert!(dr_high.v_rate < dr_low.v_rate);
    }

    #[test]
    fn test_is_default_personality_true() {
        let p = OceanProfile::default(); // all 0.5
        assert!(is_default_personality(&p));
    }

    #[test]
    fn test_is_default_personality_false() {
        let p = OceanProfile::new(0.7, 0.5, 0.5, 0.5, 0.5);
        assert!(!is_default_personality(&p));
    }

    #[test]
    fn test_is_default_personality_drifted() {
        // 模拟微小漂移 (evolve 每次 ±0.005)
        let mut p = OceanProfile::default();
        p.openness = 0.503;
        assert!(!is_default_personality(&p));
    }

    #[test]
    fn test_is_default_personality_within_tolerance() {
        // 在容差范围内 (±0.001)
        let mut p = OceanProfile::default();
        p.openness = 0.5005;
        assert!(is_default_personality(&p));
    }
}
