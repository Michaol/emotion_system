# VAD 核心数据结构与数学模型设计 (Tasks 1.1 - 1.4)

## 1. VadState 结构体

```rust
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
    pub fn new(v: f32, a: f32, d: f32) -> Self {
        Self {
            v: v.clamp(-1.0, 1.0),
            a: a.clamp(-1.0, 1.0),
            d: d.clamp(-1.0, 1.0),
        }
    }

    pub fn neutral() -> Self {
        Self { v: 0.0, a: 0.0, d: 0.0 }
    }

    /// 应用增量并 clamp
    pub fn apply_delta(&mut self, dv: f32, da: f32, dd: f32) {
        self.v = (self.v + dv).clamp(-1.0, 1.0);
        self.a = (self.a + da).clamp(-1.0, 1.0);
        self.d = (self.d + dd).clamp(-1.0, 1.0);
    }
}
```

### 内部状态容器

```rust
/// 引擎持久化存储的完整快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionState {
    pub current: VadState,
    pub baseline: VadState,
    pub personality: OceanProfile,
    pub decay_rates: DecayRates,
    pub ruminations: Vec<RuminationEntry>,
    pub stimuli_history: Vec<StimulusRecord>,
    pub last_updated_ms: i64,
}
```

---

## 2. On-Demand Decay 数学模型

### 核心公式

```
val(t) = baseline + (val(t₀) - baseline) × e^(-rate × Δt)
```

其中 `Δt = (now_ms - last_updated_ms) / 3_600_000.0` (转换为小时)。

### 半衰期 ↔ Rate 换算

```
rate = ln(2) / half_life_hours
half_life_hours = ln(2) / rate
```

### 默认半衰期参考 (来自 openfeelz)

| 维度 | Rate (/h) | 半衰期 | 备注 |
|------|-----------|--------|------|
| Valence (V) | 0.058 | ~12h | — |
| Arousal (A) | 0.087 | ~8h | 激活消退较快 |
| Dominance (D) | 0.046 | ~15h | 控制感变化缓慢 |

### Rust 实现草案

```rust
/// 按需衰减：仅在读取时计算，无后台定时器
pub fn apply_decay(current: f32, baseline: f32, rate: f32, delta_hours: f64) -> f32 {
    let factor = (-rate as f64 * delta_hours).exp() as f32;
    baseline + (current - baseline) * factor
}

/// 半衰期 → 衰减率
pub fn half_life_to_rate(half_life_hours: f32) -> f32 {
    (2.0_f32).ln() / half_life_hours
}
```

---

## 3. OCEAN 人格模块

### 数据结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OceanProfile {
    /// Openness [0.0, 1.0]
    pub openness: f32,
    /// Conscientiousness [0.0, 1.0]
    pub conscientiousness: f32,
    /// Extraversion [0.0, 1.0]
    pub extraversion: f32,
    /// Agreeableness [0.0, 1.0]
    pub agreeableness: f32,
    /// Neuroticism [0.0, 1.0]
    pub neuroticism: f32,
}

impl Default for OceanProfile {
    fn default() -> Self {
        Self {
            openness: 0.5,
            conscientiousness: 0.5,
            extraversion: 0.5,
            agreeableness: 0.5,
            neuroticism: 0.5,
        }
    }
}
```

### Baseline 调制规则

| 特质 | V baseline 调整 | A baseline 调整 | D baseline 调整 |
|------|----------------|----------------|----------------|
| Extraversion ↑ | +0.1 × (E - 0.5) | +0.15 × (E - 0.5) | +0.05 × (E - 0.5) |
| Agreeableness ↑ | +0.1 × (A - 0.5) | — | -0.05 × (A - 0.5) |
| Neuroticism ↑ | -0.15 × (N - 0.5) | +0.1 × (N - 0.5) | -0.1 × (N - 0.5) |
| Openness ↑ | +0.05 × (O - 0.5) | +0.05 × (O - 0.5) | — |
| Conscientiousness ↑ | — | -0.05 × (C - 0.5) | +0.1 × (C - 0.5) |

### Decay Rate 调制规则

| 特质 | 效果 |
|------|------|
| Neuroticism ↑ | 负面衰减 ×0.84~0.88 (负面持续更久) |
| Extraversion ↑ | V 衰减 ×1.16 (悲伤消退更快) |
| Agreeableness ↑ | V(anger) 衰减 ×1.12 (愤怒消退更快) |
| Openness ↑ | A 衰减 ×0.90 (好奇/惊叹持续更久) |

### 动态计算

```rust
pub struct DecayRates {
    pub v_rate: f32,
    pub a_rate: f32,
    pub d_rate: f32,
}

impl DecayRates {
    /// 基于人格特质调制默认衰减率
    pub fn from_personality(p: &OceanProfile) -> Self {
        let base_v = 0.058_f32;
        let base_a = 0.087_f32;
        let base_d = 0.046_f32;

        let n_mod = 1.0 - 0.16 * (p.neuroticism - 0.5).max(0.0);
        let e_mod = 1.0 + 0.16 * (p.extraversion - 0.5).max(0.0);
        let o_mod = 1.0 - 0.10 * (p.openness - 0.5).max(0.0);

        Self {
            v_rate: base_v * n_mod * e_mod,
            a_rate: base_a * o_mod,
            d_rate: base_d,
        }
    }
}
```

---

## 4. Rumination (反刍) 引擎

### 触发条件

当 `apply_event` 产生的 **任意维度** `|delta| > RUMINATION_THRESHOLD` (默认 0.3) 时，创建一条反刍条目。

### 数据结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuminationEntry {
    /// 产生反刍的事件名
    pub source_event: String,
    /// 对 V/A/D 的余波增量
    pub delta: VadState,
    /// 剩余影响轮次
    pub remaining_rounds: u32,
    /// 每轮衰减因子 (0.0 - 1.0)，如 0.7 表示每轮强度降至 70%
    pub decay_factor: f32,
}
```

### 默认参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `RUMINATION_THRESHOLD` | 0.3 | 触发阈值 |
| `max_rounds` | 5 | 最大影响轮次 |
| `decay_factor` | 0.7 | 每轮强度衰减比 |
| `max_active_entries` | 8 | 同时活跃的反刍条目上限 |

### 推进逻辑

每次调用 `get_state()` 时：

```rust
pub fn advance_ruminations(
    state: &mut VadState,
    entries: &mut Vec<RuminationEntry>,
) {
    entries.retain_mut(|entry| {
        if entry.remaining_rounds == 0 {
            return false;
        }
        let power = entry.decay_factor.powi(
            (entry.remaining_rounds.max(1)) as i32
        );
        state.apply_delta(
            entry.delta.v * power,
            entry.delta.a * power,
            entry.delta.d * power,
        );
        entry.remaining_rounds -= 1;
        entry.remaining_rounds > 0
    });
}
```

### 历史记录

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StimulusRecord {
    pub event_name: String,
    pub delta: VadState,
    pub timestamp_ms: i64,
    pub triggered_rumination: bool,
}
```
