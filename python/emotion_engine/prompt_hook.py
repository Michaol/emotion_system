"""
Prompt Hook — Agent 生命周期注入点

提供两个核心 hook:
  - before_agent_start(engine): 在 Agent 回合开始前注入 <emotion_state> XML
  - after_agent_end(engine, response): 在 Agent 回合结束后尝试分类情绪并保存

用法:
    from wrappers.python.prompt_hook import before_agent_start, after_agent_end

    prompt = before_agent_start(engine)
    # ... agent 执行 ...
    after_agent_end(engine, agent_response_text)
"""
from __future__ import annotations

import re
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    try:
        from .engine import EngineWrapper
    except (ImportError, ValueError):
        from engine import EngineWrapper


# ── 内置简易情绪词典 (无需 LLM) ──────────

_EMOTION_KEYWORDS: dict[str, list[str]] = {
    "joy":         ["happy", "glad", "delighted", "pleased", "wonderful", "开心", "高兴", "欣喜"],
    "sadness":     ["sad", "unhappy", "sorry", "depressed", "悲伤", "难过", "伤心"],
    "anger":       ["angry", "furious", "annoyed", "frustrated", "愤怒", "生气", "恼火"],
    "fear":        ["afraid", "scared", "worried", "anxious", "害怕", "担心", "焦虑"],
    "surprise":    ["surprised", "shocked", "amazed", "unexpected", "惊讶", "震惊"],
    "disgust":     ["disgusted", "revolted", "repulsed", "厌恶", "恶心"],
    "excitement":  ["excited", "thrilled", "eager", "兴奋", "激动"],
    "gratitude":   ["grateful", "thankful", "appreciate", "感谢", "感激"],
    "confusion":   ["confused", "uncertain", "puzzled", "困惑", "疑惑"],
    "contentment": ["content", "satisfied", "calm", "peaceful", "满足", "平静"],
    "curiosity":   ["curious", "interested", "intrigued", "好奇", "感兴趣"],
    "trust":       ["trust", "reliable", "depend", "信任", "可靠"],
}


def before_agent_start(engine: EngineWrapper) -> str:
    """
    Agent 回合开始前调用：生成情绪上下文 XML

    Returns:
        可直接注入到 system prompt 中的 <emotion_state> XML 字符串
    """
    return engine.format_prompt()


def after_agent_end(
    engine: EngineWrapper,
    response_text: str,
    *,
    intensity: float = 0.3,
) -> dict[str, Any] | None:
    """
    Agent 回合结束后调用：从响应文本中提取情绪并反馈到引擎

    Args:
        engine:        引擎实例
        response_text: Agent 的回复文本
        classifier:    分类器类型 ("keyword" 或未来的 "llm")
        intensity:     检测到的情绪应用强度

    Returns:
        应用事件后的 EmotionSnapshot，如果没检测到情绪则返回 None
    """
    # 目前仅支持关键词分类器
    detected = _keyword_classify(response_text)

    if detected:
        try:
            return engine.apply_event(detected, intensity)
        except ValueError:
            # 事件不在配置中，忽略
            pass

    return None


def _keyword_classify(text: str) -> str | None:
    """
    简易关键词情绪分类器

    扫描文本中的情绪关键词，返回出现频次最高的情绪标签
    """
    text_lower = text.lower()
    scores: dict[str, int] = {}

    for emotion, keywords in _EMOTION_KEYWORDS.items():
        count = sum(1 for kw in keywords if kw in text_lower)
        if count > 0:
            scores[emotion] = count

    if not scores:
        return None

    return max(scores, key=scores.get)  # type: ignore[arg-type]


def build_system_prompt_with_emotion(
    base_prompt: str,
    engine: EngineWrapper,
) -> str:
    """
    便捷函数：将情绪 XML 注入到 system prompt 末尾

    Args:
        base_prompt: 原始 system prompt
        engine:      引擎实例

    Returns:
        附带 <emotion_state> 的增强 system prompt
    """
    emotion_xml = before_agent_start(engine)
    return f"{base_prompt}\n\n{emotion_xml}"
