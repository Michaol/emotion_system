# 情绪系统构建计划 (VAD + Rust) v2.1

## 🎯 总体目标
构建一个基于 **VAD 模型**、使用 **Rust 语言** 实现的、可持久化、事件驱动、低资源占用的 **通用情绪引擎**，提供标准的 Python 与 JS/TS API，以便任意外部脚本、组件或 Agent 灵活接入。

**架构策略**：
弃用 CLI 进程调用，基于同一 Rust 核心（Cargo Workspace），通过 **PyO3** 和 **NAPI-RS** 分别编译为 Python 原生扩展和 Node.js 原生扩展，实现跨语言毫秒级纯内存互操作。

**竞品精华吸收**（来自 `openfeelz` 与 `openclaw-inner-life`）：

| 特性 | 来源 | 我们的方案 |
|------|------|-----------|
| OCEAN 人格调节 baseline 与 decay_rate | openfeelz | Rust 内置 `Personality` 模块，支持动态 `set_personality` |
| 按需衰减 (On-Demand Decay) | openfeelz | 仅在读/写时计算 `e^(-rate * Δt)`，不需后台定时器 |
| Rumination 反刍引擎 | openfeelz | 高强度情绪创建多轮余波条目，逐轮衰减释放 |
| `<emotion_state>` XML Prompt 注入 | openfeelz | 引擎输出结构化 XML，由 Hook 层注入 System Prompt |
| Agent Tool (query/modify/reset/set_personality) | openfeelz | 双语言 API 对等暴露完整 Tool 操作集 |
| 60+ 情绪标签 → 维度映射 | openfeelz | 外部 JSON 配置，支持自定义标签扩展 |
| 多 Agent 情绪感知 | openfeelz | `get_other_agents_state()` 扫描同级状态文件 |
| Brain Loop 静默调度 | inner-life | 支持 Cron 触发 NightDream / EveningReflect 脱机事件 |
| 模块化 Skill 架构 | inner-life | 引擎核心 + 可选扩展模块（reflect/dream/evolve）分层设计 |

---

## 📋 阶段一：核心设计 (Phase 1: Core Design)

### ✅ 任务清单

1.  **[设计] VAD 核心状态与数学模型**
    *   `struct VadState { v: f32, a: f32, d: f32 }`，clamped `[-1.0, 1.0]`。
    *   On-Demand Decay：`val = baseline + (cur - baseline) * e^(-rate * Δt_hours)`。
    *   记录 `last_updated_ms` (i64) 用于计算帧间时间差。
    *   产出：`design/vad_core.md`
2.  **[设计] OCEAN 人格模块**
    *   五大特质 `[0.0, 1.0]`，各自调节 V/A/D baseline 和每维度的 decay_rate。
    *   Neuroticism↑ → 负面衰减↓；Extraversion↑ → Arousal baseline↑ 等。
    *   支持运行时 `set_personality(trait, value)` 动态修改。
3.  **[设计] Rumination 反刍引擎**
    *   触发条件：单次 delta 绝对值 > 阈值。
    *   数据结构：`RuminationEntry { emotion, intensity, remaining_rounds, decay_factor }`。
    *   每次 `get_state()` 调用自动推进一轮并叠加余波。
4.  **[设计] 事件与标签映射配置 Schema**
    *   JSON 格式：`{ "event_name": { "delta_v": f32, "delta_a": f32, "delta_d": f32 } }`。
    *   支持 60+ 内置情绪标签 + 用户自定义扩展。
    *   Behavior 映射：`{ "high_arousal": { "tone": "urgent", "speed": "fast" } }`。
    *   产出：`design/emotion_config_schema.md`
5.  **[设计] 统一 API 接口 (Python & JS/TS)**
    *   `EmotionEngine` 类：`new(config_path, state_path)` / `apply_event(name, intensity?)` / `get_state()` / `modify(dimension, delta)` / `reset(dimensions?)` / `set_personality(trait, value)` / `get_personality()` / `get_other_agents(scan_dir)` / `save()` / `load()`。
    *   两端签名保持一致。
6.  **[设计] 持久化与 Prompt 注入格式**
    *   防抖落盘策略：脏标志位 + 最小间隔（如 5s），JSON 原子写入。
    *   Prompt 输出格式：`<emotion_state>` XML 块（含 dimensions 偏移、recent stimuli、rumination 状态）。

---

## 📋 阶段二：Rust 核心与 FFI 原型 (Phase 2: Prototype)

### ✅ 任务清单

1.  **[开发] 初始化 Cargo Workspace**
    *   根 `Cargo.toml` (workspace)，子 crate：`core/`、`bindings/python/`、`bindings/nodejs/`。
2.  **[开发] `core/src/vad.rs` — VAD 状态机**
    *   `VadState`、Clamp 边界、`apply_delta()`。
3.  **[开发] `core/src/decay.rs` — 按需衰减**
    *   指数衰减计算、半衰期 ↔ rate 换算工具函数。
4.  **[开发] `core/src/personality.rs` — OCEAN 人格**
    *   加载/修改人格配置、计算动态 baseline/decay_rate。
5.  **[开发] `core/src/rumination.rs` — 反刍引擎**
    *   管理活跃 RuminationEntry 队列，逐轮推进与叠加。
6.  **[开发] `core/src/config.rs` — 配置加载**
    *   解析 Event 映射 JSON 与 Behavior 映射 JSON。
7.  **[开发] `core/src/engine.rs` — 主引擎**
    *   串联完整流水线：Decay → apply_event → Rumination → Clamp → 脏标志。
8.  **[开发] `core/src/persistence.rs` — 防抖落盘**
    *   脏标志位检测、原子写入 JSON、`save()`/`load()` 接口。
9.  **[开发] `core/src/prompt.rs` — Prompt 格式化**
    *   将当前状态输出为 `<emotion_state>` XML 字符串。
10. **[开发] `core/src/multi_agent.rs` — 多 Agent 感知**
    *   扫描指定目录下其他 Agent 的状态 JSON 文件，返回摘要。
11. **[测试] `core` 单元测试**
    *   覆盖：衰减精度、Clamp 边界、人格调制、反刍轮次、配置热加载、Prompt 输出格式。
12. **[开发] `bindings/python` — PyO3 绑定**
    *   用 maturin，暴露 `EmotionEngine` 全部方法给 Python。
13. **[开发] `bindings/nodejs` — NAPI-RS 绑定**
    *   暴露 `EmotionEngine` 全部方法给 Node/TS。
14. **[测试] 跨语言集成测试**
    *   `pytest` (Python) + `vitest` (TS)，验证双端行为一致性。

---

## 📋 阶段三：通用集成与优化 (Phase 3: Integration)

### ✅ 任务清单

1.  **[集成] Python 封装层**
    *   线程/协程安全的单例 `get_engine()` 工厂函数。
2.  **[集成] Node.js 封装层**
    *   对等的单例 `getEngine()` 工厂函数。
3.  **[集成] Brain Loop 静默调度**
    *   Cron 事件分发器，支持 `NightDream` / `EveningReflect` 等内置脱机事件。
4.  **[集成] Prompt 注入 Hook**
    *   `before_agent_start`：加载状态 → 衰减 → 推进 Rumination → 格式化 XML → 注入 System Prompt。
    *   `agent_end`：根据对话内容分类情绪 → `apply_event` → 保存状态。
5.  **[优化] 性能基准测试**
    *   `apply_event` 延迟 (目标 < 1ms)、内存驻留、与 CLI 模式对比。
6.  **[文档] 集成开发指南**
    *   `docs/integration_guide.md`：Python/TS 接入示例、配置参考、Cron 模板。

---

## 📋 阶段四：迭代与扩展 (Phase 4: Expansion)

### ✅ 任务清单

1.  **[迭代] 基于实际反馈调整映射规则与衰减参数**
2.  **[扩展] 可选模块：Reflect (自反思) / Dream (创意探索) / Evolve (自进化提案)**
    *   参考 inner-life 的模块化 Skill 架构，按需安装。
3.  **[扩展] Web Dashboard (Glassmorphism UI)**
    *   实时可视化 V/A/D 维度、OCEAN 人格、Rumination 活跃条目、历史刺激。
4.  **[扩展] MCP Server**
    *   暴露 `emotion://state`、`emotion://personality` 资源给 Cursor/Claude Desktop 等 MCP 客户端。

---

## 🛠️ 技术选型

| 领域 | 选型 |
|------|------|
| 核心语言 | Rust |
| Python 绑定 | PyO3 + maturin |
| JS/TS 绑定 | NAPI-RS + @napi-rs/cli |
| 序列化 | serde + serde_json |
| 日志 | Rust `tracing`，跨语言桥接 |
| 测试 | cargo test / pytest / vitest |

---

## 📁 项目结构预览

```text
openclaw-emotion-engine/
├── Cargo.toml                # Workspace
├── core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── vad.rs            # VAD 状态 + Clamp
│       ├── decay.rs          # 按需指数衰减
│       ├── personality.rs    # OCEAN 人格
│       ├── rumination.rs     # 反刍引擎
│       ├── config.rs         # JSON 配置加载
│       ├── engine.rs         # 主引擎流水线
│       ├── persistence.rs    # 防抖落盘
│       ├── prompt.rs         # XML Prompt 格式化
│       └── multi_agent.rs    # 多 Agent 感知
├── bindings/
│   ├── python/
│   │   ├── Cargo.toml
│   │   ├── pyproject.toml
│   │   ├── src/lib.rs
│   │   └── tests/test_pyo3.py
│   └── nodejs/
│       ├── Cargo.toml
│       ├── package.json
│       ├── src/lib.rs
│       └── tests/test_napi.spec.ts
├── config/
│   ├── default_events.json   # 内置 60+ 情绪标签映射
│   └── default_behavior.json # 默认行为映射
├── design/
└── README.md
```