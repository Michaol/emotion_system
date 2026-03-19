//! Plutchik 八基本情绪模型
//!
//! 提供离散的 Plutchik 情绪状态、对立情绪自动联动、VAD 双向转换、
//! 以及 KNN 分类器用于从 VAD 坐标推断情绪标签。

use serde::{Deserialize, Serialize};

// ─── 常量 ───────────────────────────────────────────────────────────────

/// Plutchik 八基本情绪的 VAD 锚点 (Mehrabian 1996 + Russell 1980 圆环推导)
pub const ANCHORS: [(&str, f32, f32, f32); 8] = [
    ("joy", 0.81, 0.51, 0.46),
    ("trust", 0.58, 0.36, 0.28),
    ("fear", -0.64, 0.60, -0.43),
    ("surprise", 0.40, 0.67, -0.13),
    ("sadness", -0.63, -0.27, -0.33),
    ("disgust", -0.60, 0.35, 0.11),
    ("anger", -0.51, 0.59, 0.25),
    ("anticipation", 0.20, 0.40, -0.10),
];

/// 4 组对立情绪对
pub const OPPOSITES: [(&str, &str); 4] = [
    ("joy", "sadness"),
    ("trust", "disgust"),
    ("fear", "anger"),
    ("surprise", "anticipation"),
];

/// 默认对立情绪联动因子
pub const DEFAULT_OPPOSITE_FACTOR: f32 = 0.3;

// ─── PlutchikState ──────────────────────────────────────────────────────

/// Plutchik 八基本情绪状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlutchikState {
    pub joy: f32,
    pub trust: f32,
    pub fear: f32,
    pub surprise: f32,
    pub sadness: f32,
    pub disgust: f32,
    pub anger: f32,
    pub anticipation: f32,
}

impl PlutchikState {
    /// 获取指定情绪的可变引用
    fn get_mut(&mut self, name: &str) -> Option<&mut f32> {
        match name {
            "joy" => Some(&mut self.joy),
            "trust" => Some(&mut self.trust),
            "fear" => Some(&mut self.fear),
            "surprise" => Some(&mut self.surprise),
            "sadness" => Some(&mut self.sadness),
            "disgust" => Some(&mut self.disgust),
            "anger" => Some(&mut self.anger),
            "anticipation" => Some(&mut self.anticipation),
            _ => None,
        }
    }

    /// 获取指定情绪的值
    #[must_use]
    pub fn get(&self, name: &str) -> Option<f32> {
        match name {
            "joy" => Some(self.joy),
            "trust" => Some(self.trust),
            "fear" => Some(self.fear),
            "surprise" => Some(self.surprise),
            "sadness" => Some(self.sadness),
            "disgust" => Some(self.disgust),
            "anger" => Some(self.anger),
            "anticipation" => Some(self.anticipation),
            _ => None,
        }
    }

    /// 更新单个情绪 (clamp 到 [0, 1])
    pub fn update(&mut self, name: &str, delta: f32) {
        if let Some(val) = self.get_mut(name) {
            *val = (*val + delta).clamp(0.0, 1.0);
        }
    }

    /// 更新情绪并自动联动对立面
    /// opposite_delta = -delta * opposite_factor
    pub fn update_with_opposite(&mut self, name: &str, delta: f32, opposite_factor: f32) {
        self.update(name, delta);
        // 查找对立面
        if let Some((_, opp)) = OPPOSITES.iter().find(|(a, _)| *a == name) {
            self.update(opp, -delta * opposite_factor);
        } else if let Some((a, _)) = OPPOSITES.iter().find(|(_, b)| *b == name) {
            self.update(a, -delta * opposite_factor);
        }
    }

    /// 返回 (emotion_name, intensity) 最强情绪
    #[must_use]
    pub fn dominant(&self) -> (&str, f32) {
        let emotions = [
            ("joy", self.joy),
            ("trust", self.trust),
            ("fear", self.fear),
            ("surprise", self.surprise),
            ("sadness", self.sadness),
            ("disgust", self.disgust),
            ("anger", self.anger),
            ("anticipation", self.anticipation),
        ];
        emotions
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(n, v)| (*n, *v))
            .unwrap_or(("neutral", 0.0))
    }

    /// 将 Plutchik 状态转换为加权 VAD (按各情绪强度加权锚点)
    #[must_use]
    pub fn to_vad(&self) -> crate::vad::VadState {
        let weights = [
            self.joy,
            self.trust,
            self.fear,
            self.surprise,
            self.sadness,
            self.disgust,
            self.anger,
            self.anticipation,
        ];
        let total: f32 = weights.iter().sum();
        if total < 1e-6 {
            return crate::vad::VadState::neutral();
        }
        let mut v = 0.0f32;
        let mut a = 0.0f32;
        let mut d = 0.0f32;
        for (i, (_, av, aa, ad)) in ANCHORS.iter().enumerate() {
            v += weights[i] * av;
            a += weights[i] * aa;
            d += weights[i] * ad;
        }
        crate::vad::VadState::new(v / total, a / total, d / total)
    }

    /// 重置所有情绪到 0
    pub fn reset(&mut self) {
        self.joy = 0.0;
        self.trust = 0.0;
        self.fear = 0.0;
        self.surprise = 0.0;
        self.sadness = 0.0;
        self.disgust = 0.0;
        self.anger = 0.0;
        self.anticipation = 0.0;
    }
}

impl Default for PlutchikState {
    fn default() -> Self {
        Self {
            joy: 0.0,
            trust: 0.0,
            fear: 0.0,
            surprise: 0.0,
            sadness: 0.0,
            disgust: 0.0,
            anger: 0.0,
            anticipation: 0.0,
        }
    }
}

// ─── KNN 分类器 ─────────────────────────────────────────────────────────

/// KNN 分类结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlutchikResult {
    pub label: String,
    pub confidence: f32,
}

/// KNN 分类：从 VAD 坐标推断 Plutchik 情绪标签
#[must_use]
pub fn classify_plutchik(current: &crate::vad::VadState, k: usize) -> PlutchikResult {
    // 1. 计算到所有锚点的距离
    let mut distances: [(&str, f32); 8] = ANCHORS.map(|(name, va, aa, da)| {
        let dv = current.v - va;
        let da2 = current.a - aa;
        let dd = current.d - da;
        let dist = (dv * dv + da2 * da2 + dd * dd).sqrt();
        (name, dist)
    });

    // 2. 按距离排序，取 K 个最近邻
    distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let k = k.min(8);

    // 3. 距离加权投票 (栈上固定数组，8 种情绪)
    let mut vote_names: [&str; 8] = [""; 8];
    let mut vote_weights: [f32; 8] = [0.0; 8];
    let mut vote_count: usize = 0;

    for (name, dist) in &distances[..k] {
        let weight = 1.0 / (1.0 + dist);
        // 查找是否已有该情绪的投票
        if let Some(pos) = (0..vote_count).find(|&i| vote_names[i] == *name) {
            vote_weights[pos] += weight;
        } else {
            vote_names[vote_count] = name;
            vote_weights[vote_count] = weight;
            vote_count += 1;
        }
    }

    // 4. 选出最高票
    let mut best_idx = 0;
    let mut best_weight = vote_weights[0];
    for i in 1..vote_count {
        if vote_weights[i] > best_weight {
            best_idx = i;
            best_weight = vote_weights[i];
        }
    }

    // 5. 置信度 = 最高票权重 / 总权重
    let total: f32 = vote_weights[..vote_count].iter().sum();
    let confidence = if total > 1e-6 {
        best_weight / total
    } else {
        0.0
    };

    PlutchikResult {
        label: vote_names[best_idx].to_string(),
        confidence,
    }
}

// ─── 测试 ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- PlutchikState 测试 --

    #[test]
    fn test_plutchik_state_default_is_neutral() {
        let s = PlutchikState::default();
        assert_eq!(s.joy, 0.0);
        assert_eq!(s.anger, 0.0);
        assert_eq!(s.anticipation, 0.0);
    }

    #[test]
    fn test_update_single_emotion() {
        let mut s = PlutchikState::default();
        s.update("joy", 0.5);
        assert!((s.joy - 0.5).abs() < 1e-6);
        assert_eq!(s.sadness, 0.0); // 未联动
    }

    #[test]
    fn test_update_with_opposite() {
        let mut s = PlutchikState::default();
        s.update_with_opposite("joy", 0.6, 0.3);
        assert!((s.joy - 0.6).abs() < 1e-6);
        let expected_sadness = (-0.6_f32 * 0.3).max(0.0);
        assert!((s.sadness - expected_sadness).abs() < 1e-6);
    }

    #[test]
    fn test_update_clamps_to_unit_range() {
        let mut s = PlutchikState::default();
        s.update("fear", 2.0);
        assert_eq!(s.fear, 1.0);

        s.update("fear", -3.0);
        assert_eq!(s.fear, 0.0);
    }

    #[test]
    fn test_dominant_emotion() {
        let mut s = PlutchikState::default();
        s.trust = 0.8;
        s.joy = 0.3;
        let (name, val) = s.dominant();
        assert_eq!(name, "trust");
        assert!((val - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_dominant_all_zero() {
        let s = PlutchikState::default();
        let (name, val) = s.dominant();
        // 全零时 max_by 返回任一 tied 元素，只验证值为 0
        let valid = [
            "joy",
            "trust",
            "fear",
            "surprise",
            "sadness",
            "disgust",
            "anger",
            "anticipation",
        ];
        assert!(valid.contains(&name));
        assert!((val - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_to_vad_joy() {
        let mut s = PlutchikState::default();
        s.joy = 1.0;
        let vad = s.to_vad();
        // joy anchor: (0.81, 0.51, 0.46)
        assert!((vad.v - 0.81).abs() < 0.05);
        assert!((vad.a - 0.51).abs() < 0.05);
        assert!((vad.d - 0.46).abs() < 0.05);
    }

    #[test]
    fn test_to_vad_neutral() {
        let s = PlutchikState::default();
        let vad = s.to_vad();
        assert!(vad.v.abs() < 0.01);
        assert!(vad.a.abs() < 0.01);
        assert!(vad.d.abs() < 0.01);
    }

    #[test]
    fn test_reset() {
        let mut s = PlutchikState::default();
        s.joy = 0.8;
        s.anger = 0.5;
        s.reset();
        assert_eq!(s.joy, 0.0);
        assert_eq!(s.anger, 0.0);
    }

    #[test]
    fn test_opposite_pairs() {
        // joy ↔ sadness: joy 增加 → sadness 应被联动减少 (从 0 开始 clamp 到 0)
        let mut s = PlutchikState::default();
        s.update_with_opposite("joy", 0.7, 0.3);
        assert!(s.joy > 0.6);
        // sadness 从 0.0 减去 0.21 → clamp 到 0.0
        assert!(s.sadness.abs() < 1e-6);

        // 反向测试: sadness 增加 → joy 减少
        let mut s2 = PlutchikState::default();
        s2.joy = 0.5;
        s2.update_with_opposite("sadness", 0.7, 0.3);
        assert!(s2.sadness > 0.6);
        assert!(s2.joy < 0.5); // joy 被联动减少
    }

    // -- KNN 分类器测试 --

    #[test]
    fn test_knn_classify_joy() {
        let vad = crate::vad::VadState::new(0.8, 0.5, 0.4);
        let result = classify_plutchik(&vad, 3);
        assert_eq!(result.label, "joy");
        assert!(result.confidence > 0.3);
    }

    #[test]
    fn test_knn_classify_fear() {
        let vad = crate::vad::VadState::new(-0.6, 0.6, -0.4);
        let result = classify_plutchik(&vad, 3);
        assert_eq!(result.label, "fear");
    }

    #[test]
    fn test_knn_classify_sadness() {
        let vad = crate::vad::VadState::new(-0.6, -0.3, -0.3);
        let result = classify_plutchik(&vad, 3);
        assert_eq!(result.label, "sadness");
    }

    #[test]
    fn test_knn_classify_anger() {
        let vad = crate::vad::VadState::new(-0.5, 0.6, 0.3);
        let result = classify_plutchik(&vad, 3);
        assert_eq!(result.label, "anger");
    }

    #[test]
    fn test_knn_classify_trust() {
        let vad = crate::vad::VadState::new(0.6, 0.3, 0.3);
        let result = classify_plutchik(&vad, 3);
        assert_eq!(result.label, "trust");
    }

    #[test]
    fn test_knn_classify_anticipation() {
        let vad = crate::vad::VadState::new(0.2, 0.4, -0.1);
        let result = classify_plutchik(&vad, 3);
        assert_eq!(result.label, "anticipation");
    }

    #[test]
    fn test_knn_neutral_low_confidence() {
        let vad = crate::vad::VadState::neutral();
        let result = classify_plutchik(&vad, 3);
        // 中性点离所有锚点都较远，confidence 应较低
        assert!(result.confidence < 0.5);
    }

    #[test]
    fn test_knn_returns_valid_emotion() {
        let vad = crate::vad::VadState::new(0.1, 0.2, -0.1);
        let result = classify_plutchik(&vad, 5);
        let valid = [
            "joy",
            "trust",
            "fear",
            "surprise",
            "sadness",
            "disgust",
            "anger",
            "anticipation",
        ];
        assert!(valid.contains(&result.label.as_str()));
    }
}
