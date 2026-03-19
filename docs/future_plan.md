# Emotion Engine Future Plans

> v2.0.0 已实现: Plutchik 八基本情绪、KNN 分类、情感记忆耦合。

## 已实现 (v2.0.0)

- PlutchikState (8 emotions) + 对立情绪自动联动
- KNN VAD-to-Plutchik 分类器
- EmotionalMemory (幂律衰减 + 召回强化 + 显著性加权)
- `<plutchik>` + `<memories>` XML 节点

## 待实现

### 1. 双速衰减 (Dual-Speed Decay)
参考 Sentipolis (CMU, 2026)
- 快速更新: 每轮对话即时计算
- 慢速更新: reflect() 时整合长期模式

### 2. 语义丰富层 (Semantic Enrichment)
- VAD → KNN → label → vivid_paragraph → inject into prompt
- 生成自然语言情绪描述替代原始数值

### 3. Plutchik Dyad 组合
28 种二元复合情绪检测:
- Love = Joy + Trust
- Curiosity = Surprise + Anticipation
- Submission = Trust + Fear
- Awe = Fear + Surprise
- Optimism = Anticipation + Joy

### 4. 半衰期衰减选项
- 新增 `decay_policy: "half_life"` 配置
- T_1/2 = 120 分钟 (Sentipolis 实验值)
