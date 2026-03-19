# 情绪映射配置 Schema 设计 (Tasks 1.5 - 1.6)

## 1. 事件映射 Schema (`default_events.json`)

外部 JSON 配置文件，定义事件名到 VAD 增量的映射。

### 格式

```json
{
  "version": "1.0",
  "events": {
    "joy": { "delta_v": 0.4, "delta_a": 0.2, "delta_d": 0.1 },
    "anger": { "delta_v": -0.5, "delta_a": 0.6, "delta_d": 0.3 },
    "fear": { "delta_v": -0.4, "delta_a": 0.5, "delta_d": -0.4 },
    "sadness": { "delta_v": -0.4, "delta_a": -0.3, "delta_d": -0.2 },
    "surprise": { "delta_v": 0.1, "delta_a": 0.6, "delta_d": 0.0 },
    "disgust": { "delta_v": -0.4, "delta_a": 0.2, "delta_d": 0.2 },
    "...": "..."
  }
}
```

### Rust 对应结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    pub version: String,
    pub events: HashMap<String, EventDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDelta {
    pub delta_v: f32,
    pub delta_a: f32,
    pub delta_d: f32,
}
```

### 强度缩放

`apply_event(name, intensity)` 调用时：
```
actual_delta = config_delta × intensity
```
`intensity` 默认 1.0，范围 `[0.0, 2.0]`。

---

## 2. 行为映射 Schema (`default_behavior.json`)

定义 VAD 区间到输出行为/语气的映射，用于 Prompt 注入时的自然语言描述。

### 格式

```json
{
  "version": "1.0",
  "behaviors": [
    {
      "condition": { "v_min": 0.3, "a_min": 0.5 },
      "tone": "enthusiastic",
      "speed": "fast",
      "description": "高兴且激动"
    },
    {
      "condition": { "v_max": -0.3, "a_min": 0.5 },
      "tone": "agitated",
      "speed": "fast",
      "description": "愤怒或焦虑"
    },
    {
      "condition": { "v_max": -0.3, "a_max": -0.2 },
      "tone": "melancholic",
      "speed": "slow",
      "description": "低落且消沉"
    },
    {
      "condition": { "v_min": 0.3, "a_max": 0.0 },
      "tone": "warm",
      "speed": "moderate",
      "description": "平静且愉悦"
    }
  ],
  "default": {
    "tone": "neutral",
    "speed": "moderate",
    "description": "情绪平稳"
  }
}
```

### Rust 对应结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    pub version: String,
    pub behaviors: Vec<BehaviorRule>,
    pub default: BehaviorOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorCondition {
    #[serde(default)] pub v_min: Option<f32>,
    #[serde(default)] pub v_max: Option<f32>,
    #[serde(default)] pub a_min: Option<f32>,
    #[serde(default)] pub a_max: Option<f32>,
    #[serde(default)] pub d_min: Option<f32>,
    #[serde(default)] pub d_max: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorRule {
    pub condition: BehaviorCondition,
    #[serde(flatten)]
    pub output: BehaviorOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorOutput {
    pub tone: String,
    pub speed: String,
    pub description: String,
}
```

### 匹配逻辑

按配置文件中的顺序遍历 `behaviors`，第一个满足所有 `condition` 字段的规则命中；若全部未命中则使用 `default`。
