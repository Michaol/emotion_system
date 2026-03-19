# Emotion Engine 2.0.0

[中文](#中文) | [English](#english) | [🤖 For AI Agents](FOR_AI.md)

> [!IMPORTANT]
> **🚀 觉得安装/配置太复杂看不懂？这不重要！**  
> 请直接将此项目链接或 [FOR_AI.md](FOR_AI.md) 发送给您的 AI 代理。**它比你聪明，它能根据文档全自动完成所有部署和集成工作。**

---

## 中文

### 项目简介
本项目是一个基于 Rust 核心开发的高性能情绪模拟引擎。采用 **VAD (Valence-Arousal-Dominance)** 三维情绪空间模型，为 AI 代理 (Agents) 提供动态情绪演化、性格漂移、自我反思以及 **Plutchik 八基本情绪分类** 与 **情感记忆耦合** 能力。

### v2.0.0 新特性

- **Plutchik 八基本情绪模型**: 在 VAD 连续坐标轴基础上，新增离散的八种基本情绪分类（喜、信、惧、惊、哀、厌、怒、期），支持对立情绪自动联动。
- **KNN 情绪分类器**: 基于 K-Nearest Neighbors 算法，从 VAD 坐标推断 Plutchik 标签及置信度。
- **情感记忆耦合 (Emotion-Memory Coupling)**: 事件附带情绪标签存储，基于 cosine similarity 检索，支持幂律衰减和召回强化。
- **语义丰富 Prompt**: XML 输出新增 `<plutchik>` 和 `<memories>` 节点，为 LLM 提供更丰富的情绪上下文。
- **零新增依赖**: 所有新功能基于已有 serde/std 库实现，无额外外部依赖。

### 核心特性

| 特性 | 说明 |
|------|------|
| **VAD 三维情绪建模** | Valence (愉悦/悲伤)、Arousal (激活/平静)、Dominance (支配/顺从) |
| **Plutchik 八情绪** | 喜 (Joy)、信 (Trust)、惧 (Fear)、惊 (Surprise)、哀 (Sadness)、厌 (Disgust)、怒 (Anger)、期 (Anticipation) |
| **对立情绪联动** | 更新一种情绪时自动影响其对立面 (如 喜↔哀, 信↔厌) |
| **OCEAN 人格驱动** | 大五人格特质决定情绪基线和衰减率 |
| **按需衰减** | 仅在读取时计算，极低 CPU 占用 |
| **情感记忆** | 幂律衰减 + 召回强化 + 显著性加权 |
| **反刍引擎** | 高强度事件产生多轮余波效应 |
| **MCP 原生** | FastMCP 实现，支持 OpenClaw 动态注入 |

### 安装与编译

确保系统中已安装 Rust (Cargo) 和 Python 3.10+。

```powershell
# 方式 1: 从 GitHub 安装 (推荐)
pip install git+https://github.com/Michaol/emotion_system.git

# 方式 2: 本地编译
git clone https://github.com/Michaol/emotion_system.git
cd emotion_system
pip install maturin
maturin develop --release
```

### OpenClaw MCP 接入

在 OpenClaw 的 `mcp_config.json` 中配置情绪引擎。

#### 场景 1：多代理隔离模式 (推荐)
分别为每个 Agent 分配独立的 MCP 服务名和环境 ID。
```json
{
  "mcpServers": {
    "emotion-alice": {
      "command": "emotion-mcp",
      "env": { "EMOTION_AGENT_ID": "Alice", "EMOTION_STATE_DIR": "./state" }
    },
    "emotion-bob": {
      "command": "emotion-mcp",
      "env": { "EMOTION_AGENT_ID": "Bob", "EMOTION_STATE_DIR": "./state" }
    }
  }
}
```

#### 场景 2：共享中心模式
启动一个通用服务，在工具调用时动态传递 `agent_id`。
```json
{
  "mcpServers": {
    "emotion-hub": {
      "command": "emotion-mcp",
      "env": { "EMOTION_STATE_DIR": "./state" }
    }
  }
}
```

#### 方案对比

| 维度 | **方案 A：一代理一服务** | **方案 B：共享中心模式** |
| :--- | :--- | :--- |
| **隔离性** | 进程级隔离 | 软隔离 |
| **资源开销** | 每 Agent ~50-100MB | 单进程 |
| **LLM 体验** | 工具名自动前缀 | 需正确填写 ID |
| **故障影响** | 局部 | 全局 |
| **适用场景** | 精品代理 | 大规模集群 |

### 人设管理

- **增加**: 在 `config/` 下创建 `{agent_id}.json`，填入 OCEAN 属性 (0.0~1.0)
- **修改**: 编辑 `config/{agent_id}.json` (初始) 或 `state/{agent_id}.json` (演进状态)
- **重置**: 调用 `reset` 工具或删除 `state/{agent_id}.json`
- **删除**: 同时移除 `config/` 和 `state/` 下的同名 JSON

### MCP 接口参考

#### 可用工具 (Tools)

| 工具名称 | 功能 | 参数 |
| :--- | :--- | :--- |
| `apply_emotion_event` | 应用情绪事件 | `event_name`, `intensity`, `agent_id` |
| `modify_emotion_dimension` | 手动校准维度 | `dimension` (v/a/d), `delta`, `agent_id` |
| `reset` | 状态重置 | `agent_id` |
| `reflect` | 强制反思 | `agent_id` |
| `dream` | 潜意识梦境 | `agent_id` |
| `evolve` | 性格演化 | `agent_id` |

#### 可用资源 (Resources)

- `emotion://state/{agent_id}` — 实时 VAD 状态与 Plutchik 分类 (JSON)
- `emotion://prompt/{agent_id}` — 包含 `<plutchik>` 和 `<memories>` 的 XML 上下文
- `emotion://personality/{agent_id}` — OCEAN 五维度人格
- `emotion://reflect/{agent_id}` — 最新反思文本

### Web 看板

```powershell
emotion-bridge
# 访问 http://localhost:8000
```

---

## English

### Overview
A high-performance emotion simulation engine with a **Rust core**. Implements **VAD (Valence-Arousal-Dominance)** modeling with **Plutchik 8-basic-emotion classification** and **emotion-memory coupling** for AI Agents.

### v2.0.0 Highlights

- **Plutchik Model**: 8 discrete emotions (Joy, Trust, Fear, Surprise, Sadness, Disgust, Anger, Anticipation) with automatic opposite linkage.
- **KNN Classifier**: K-Nearest Neighbors mapping from VAD coordinates to Plutchik labels with confidence scores.
- **Emotion-Memory Coupling**: Events stored with emotion tags, power-law decay, recall reinforcement, cosine-similarity retrieval.
- **Rich Prompt XML**: New `<plutchik>` and `<memories>` nodes for LLM context enrichment.

### Installation

```powershell
pip install git+https://github.com/Michaol/emotion_system.git

# Or local build
pip install maturin
maturin develop --release
```

### Web Dashboard

```powershell
emotion-bridge
# Visit http://localhost:8000
```

---

## License

**CC BY-NC 4.0 (Attribution-NonCommercial 4.0 International)**

- **Permitted**: Attribution, Non-Commercial use, Modification, Distribution.
- **Forbidden**: Any commercial use for profit purposes.
