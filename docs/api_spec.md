# 统一 API、持久化与 Prompt 注入设计 (Tasks 1.7 - 1.9)

## 1. EmotionEngine 统一 API 签名

Python 与 JS/TS 两端方法签名保持一致。

### 构造与生命周期

| 方法 | Python 签名 | TS 签名 | 说明 |
|------|------------|---------|------|
| 构造 | `EmotionEngine(config_path: str, state_path: str)` | `new EmotionEngine(configPath: string, statePath: string)` | 加载配置 + 恢复状态 |
| 销毁 | `engine.save()` (显式) | `engine.save()` (显式) | 解构前手动落盘 |

### 核心读写

| 方法 | 签名 | 返回 | 说明 |
|------|------|------|------|
| `apply_event` | `(name: str, intensity: float = 1.0)` | `EmotionSnapshot` | 查配置 → 缩放增量 → 应用 → 检查反刍 → 标脏 |
| `get_state` | `()` | `EmotionSnapshot` | 按需衰减 → 推进反刍 → 返回快照 (不标脏) |
| `modify` | `(dimension: str, delta: float)` | `EmotionSnapshot` | 直接修改单个维度 |
| `reset` | `(dimensions: list[str] | None = None)` | `EmotionSnapshot` | 重置指定维度或全部至 baseline |

### 人格管理

| 方法 | 签名 | 返回 |
|------|------|------|
| `set_personality` | `(trait_name: str, value: float)` | `None` |
| `get_personality` | `()` | `OceanProfile` |

### 多 Agent 感知

| 方法 | 签名 | 返回 |
|------|------|------|
| `get_other_agents` | `(scan_dir: str)` | `list[AgentSummary]` |

### Prompt 与持久化

| 方法 | 签名 | 返回 |
|------|------|------|
| `format_prompt` | `()` | `str` (XML 块) |
| `save` | `()` | `None` |
| `load` | `()` | `None` |

### 返回类型定义

```python
# Python (dataclass-like, 由 PyO3 转换)
class EmotionSnapshot:
    v: float
    a: float
    d: float
    dominant_emotion: str       # 当前最匹配的情绪标签
    tone: str                   # 行为映射输出
    active_ruminations: int     # 活跃反刍数

class OceanProfile:
    openness: float
    conscientiousness: float
    extraversion: float
    agreeableness: float
    neuroticism: float

class AgentSummary:
    agent_id: str
    v: float
    a: float
    d: float
    last_updated: str           # ISO 8601
```

```typescript
// TypeScript (自动由 NAPI-RS 生成 .d.ts)
interface EmotionSnapshot {
  v: number;
  a: number;
  d: number;
  dominantEmotion: string;
  tone: string;
  activeRuminations: number;
}

interface OceanProfile {
  openness: number;
  conscientiousness: number;
  extraversion: number;
  agreeableness: number;
  neuroticism: number;
}
```

### PyO3 绑定草案

```rust
#[pyclass]
pub struct EmotionEngine {
    inner: core::Engine,
}

#[pymethods]
impl EmotionEngine {
    #[new]
    #[pyo3(signature = (config_path, state_path))]
    fn new(config_path: &str, state_path: &str) -> PyResult<Self> { ... }

    fn apply_event(&mut self, name: &str, intensity: Option<f32>) -> PyResult<EmotionSnapshot> { ... }
    fn get_state(&mut self) -> PyResult<EmotionSnapshot> { ... }
    fn modify(&mut self, dimension: &str, delta: f32) -> PyResult<EmotionSnapshot> { ... }
    fn reset(&mut self, dimensions: Option<Vec<String>>) -> PyResult<EmotionSnapshot> { ... }
    fn set_personality(&mut self, trait_name: &str, value: f32) -> PyResult<()> { ... }
    fn get_personality(&self) -> PyResult<OceanProfile> { ... }
    fn format_prompt(&mut self) -> PyResult<String> { ... }
    fn save(&self) -> PyResult<()> { ... }
    fn load(&mut self) -> PyResult<()> { ... }
}
```

### NAPI-RS 绑定草案

```rust
#[napi]
pub struct EmotionEngine {
    inner: core::Engine,
}

#[napi]
impl EmotionEngine {
    #[napi(constructor)]
    pub fn new(config_path: String, state_path: String) -> napi::Result<Self> { ... }

    #[napi]
    pub fn apply_event(&mut self, name: String, intensity: Option<f64>) -> napi::Result<EmotionSnapshot> { ... }

    #[napi]
    pub fn get_state(&mut self) -> napi::Result<EmotionSnapshot> { ... }

    #[napi]
    pub fn format_prompt(&mut self) -> napi::Result<String> { ... }

    #[napi]
    pub fn save(&self) -> napi::Result<()> { ... }
}
```

---

## 2. 防抖持久化策略

### 机制

| 组件 | 实现 |
|------|------|
| 脏标志 | `dirty: bool`，`apply_event` / `modify` / `reset` 设为 `true` |
| 最小间隔 | `MIN_SAVE_INTERVAL = 5s`，两次写入之间至少间隔 5 秒 |
| 原子写入 | 先写临时文件 `.state.tmp`，成功后 `rename` 为目标路径 |
| 显式调用 | `save()` 强制立即写入，忽略间隔限制 |
| 崩溃恢复 | `load()` 时如果 `.state.tmp` 存在且主文件损坏，从 tmp 恢复 |

### 状态文件格式 (`agent_state.json`)

```json
{
  "current": { "v": 0.35, "a": -0.12, "d": 0.08 },
  "baseline": { "v": 0.05, "a": 0.0, "d": 0.0 },
  "personality": {
    "openness": 0.7, "conscientiousness": 0.6,
    "extraversion": 0.5, "agreeableness": 0.8, "neuroticism": 0.3
  },
  "decay_rates": { "v_rate": 0.058, "a_rate": 0.087, "d_rate": 0.046 },
  "ruminations": [],
  "stimuli_history": [],
  "last_updated_ms": 1742318400000
}
```

---

## 3. Prompt 注入格式 (`<emotion_state>` XML)

### 输出样例

```xml
<emotion_state>
  <dimensions>
    valence: elevated (0.35), arousal: lowered (-0.12), dominance: neutral (0.08)
  </dimensions>
  <tone>warm, moderate speed</tone>
  <rumination active="1">
    Processing residual frustration from recent deployment failure (2 rounds remaining)
  </rumination>
  <stimuli recent="3">
    2026-03-18 21:00: Felt moderately joyful because user praised work.
    2026-03-18 20:30: Felt mildly frustrated because test suite failed.
    2026-03-18 20:00: Felt curious because new library discovered.
  </stimuli>
  <others>
    research-agent: valence=0.1, arousal=0.4 (2026-03-18 20:45)
  </others>
</emotion_state>
```

### 生成规则

| 节 | 条件 | 内容 |
|----|------|------|
| `<dimensions>` | 总是输出 | 仅列出偏离 baseline > 0.15 的维度 |
| `<tone>` | 总是输出 | 行为映射匹配结果 |
| `<rumination>` | 存在活跃条目 | 描述来源事件和剩余轮次 |
| `<stimuli>` | 历史不空 | 最近 3 条刺激记录 |
| `<others>` | 扫描到其他 Agent | 其他 Agent 摘要 |

### Rust 实现接口

```rust
impl Engine {
    pub fn format_prompt(&mut self) -> String {
        // 1. 先 apply_decay
        // 2. 再 advance_ruminations
        // 3. 匹配 behavior
        // 4. 拼装 XML 字符串
        todo!()
    }
}
```
