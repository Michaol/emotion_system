use serde::{Deserialize, Serialize};

/// VAD 情绪状态向量，所有值 clamp 到 [-1.0, 1.0]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VadState {
    /// Valence: 愉悦 (+) / 不悦 (-)
    pub v: f32,
    /// Arousal: 激活 (+) / 平静 (-)
    pub a: f32,
    /// Dominance: 支配 (+) / 顺从 (-)
    pub d: f32,
}

impl VadState {
    /// 创建一个 clamped 的 VAD 状态
    #[must_use]
    pub fn new(v: f32, a: f32, d: f32) -> Self {
        Self {
            v: v.clamp(-1.0, 1.0),
            a: a.clamp(-1.0, 1.0),
            d: d.clamp(-1.0, 1.0),
        }
    }

    /// 中性状态 (0, 0, 0)
    #[must_use]
    pub fn neutral() -> Self {
        Self {
            v: 0.0,
            a: 0.0,
            d: 0.0,
        }
    }

    /// 应用增量并 clamp
    pub fn apply_delta(&mut self, dv: f32, da: f32, dd: f32) {
        self.v = (self.v + dv).clamp(-1.0, 1.0);
        self.a = (self.a + da).clamp(-1.0, 1.0);
        self.d = (self.d + dd).clamp(-1.0, 1.0);
    }

    /// 返回三个维度中绝对值最大的那个
    #[must_use]
    pub fn max_abs(&self) -> f32 {
        self.v.abs().max(self.a.abs()).max(self.d.abs())
    }
}

impl Default for VadState {
    fn default() -> Self {
        Self::neutral()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_clamps_values() {
        let s = VadState::new(1.5, -2.0, 0.5);
        assert_eq!(s.v, 1.0);
        assert_eq!(s.a, -1.0);
        assert_eq!(s.d, 0.5);
    }

    #[test]
    fn test_apply_delta_clamps() {
        let mut s = VadState::new(0.9, -0.8, 0.0);
        s.apply_delta(0.5, -0.5, 0.0);
        assert_eq!(s.v, 1.0);
        assert_eq!(s.a, -1.0);
        assert_eq!(s.d, 0.0);
    }

    #[test]
    fn test_neutral() {
        let s = VadState::neutral();
        assert_eq!(s.v, 0.0);
        assert_eq!(s.a, 0.0);
        assert_eq!(s.d, 0.0);
    }

    #[test]
    fn test_max_abs() {
        let s = VadState::new(0.3, -0.7, 0.5);
        assert!((s.max_abs() - 0.7).abs() < f32::EPSILON);
    }
}
