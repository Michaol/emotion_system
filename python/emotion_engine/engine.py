"""
emotion_engine — 线程安全 & 协程安全的 Python 封装层

内置模块:
  - Reflect:  定期回顾情绪状态，输出自我认知文本
  - Dream:    空闲期随机微弱情绪波动，模拟潜意识整理
  - Evolve:   长期统计刺激频率，微调人格特质 (drift)

用法:
    from wrappers.python.emotion_engine import get_engine

    engine = get_engine()           # 获取/创建全局单例
    snap   = engine.apply_event("joy", 0.8)
    prompt = engine.format_prompt()
    report = engine.reflect()       # 手动触发反思
    dream  = engine.dream()         # 手动触发梦境
    drift  = engine.evolve()        # 手动触发人格漂移
"""

import os
import random
import threading
import time
from pathlib import Path
from typing import Any

from emotion_engine._core import EmotionEngine as _RustEngine

# ── 自动路径解析 ──────────────────────────
_PACKAGE_ROOT = Path(__file__).resolve().parent


def _find_default(name: str, rel_dev: str) -> str:
    """按优先级寻找目录：1. 环境变量 2. 包内资源 (site-packages) 3. 开发环境 (源码)"""
    # 1. 检查当前目录下 (比如被打包进 whl 的资源)
    pkg_path = _PACKAGE_ROOT / name
    if pkg_path.exists():
        return str(pkg_path)
    # 2. 检查开发源码目录下 (通常在 ../../ 下)
    dev_path = _PACKAGE_ROOT.parents[1] / rel_dev
    if dev_path.exists():
        return str(dev_path)
    # 3. 如果都不存在，默认为预期开发路径，避免运行期 None 错误
    return str(_PACKAGE_ROOT.parents[1] / rel_dev)


_DEFAULT_CONFIG_DIR = os.environ.get(
    "EMOTION_CONFIG_DIR", _find_default("config", "config")
)
_DEFAULT_STATE_DIR = os.environ.get(
    "EMOTION_STATE_DIR", _find_default("state", "state")
)
_DEFAULT_AGENT_ID = os.environ.get("EMOTION_AGENT_ID", "default")

# ── 单例管理 ──────────────────────────────

_lock = threading.Lock()
_instances: dict[str, EngineWrapper] = {}


# ── Dream 配置 ────────────────────────────

_DREAM_THEMES = [
    {"theme": "nostalgic_memory", "v": -0.05, "a": -0.08, "d": -0.02},
    {"theme": "hopeful_vision", "v": 0.08, "a": 0.03, "d": 0.05},
    {"theme": "anxious_replay", "v": -0.06, "a": 0.10, "d": -0.08},
    {"theme": "peaceful_scene", "v": 0.06, "a": -0.10, "d": 0.03},
    {"theme": "creative_spark", "v": 0.04, "a": 0.06, "d": 0.02},
    {"theme": "processing_grief", "v": -0.08, "a": -0.05, "d": -0.05},
]

# ── Evolve 配置 ───────────────────────────

_DRIFT_MAP: dict[str, dict[str, float]] = {
    "positive": {
        "extraversion": +0.002,
        "agreeableness": +0.001,
        "neuroticism": -0.001,
    },
    "negative": {"neuroticism": +0.002, "agreeableness": -0.001},
    "high_arousal": {"openness": +0.001, "neuroticism": +0.001},
    "high_dominance": {"conscientiousness": +0.001, "extraversion": +0.001},
}
_MAX_DRIFT_PER_CALL = 0.005


class EngineWrapper:
    """线程安全的引擎封装：所有写操作自动加锁，内置 Reflect/Dream/Evolve"""

    def __init__(self, agent_id: str, config_dir: str, state_path: str) -> None:
        self._lock = threading.Lock()
        self._engine = _RustEngine(config_dir, state_path, agent_id)
        self._agent_id = agent_id
        self._state_dir = os.path.dirname(state_path)
        self._last_save_ms = 0
        self._save_interval_ms = 5000  # 防抖间隔

        # ── Reflect ──────────────────────
        self._reflect_interval = 5  # 每 N 次 apply_event 触发
        self._reflect_counter = 0
        self._last_reflect: str | None = None

        # ── Evolve ───────────────────────
        self._evolve_history: list[dict[str, float]] = []

        # ── Decay Loop ───────────────────
        self._decay_thread: threading.Thread | None = None
        self._decay_stop = threading.Event()

    # ═══════════════════════════════════════
    #  读操作
    # ═══════════════════════════════════════

    def get_state(self) -> dict[str, Any]:
        """获取当前情绪状态快照，合并由 Python 维护的元数据 (Reflect, ID 等)"""
        with self._lock:
            snap = self._engine.get_state()
            snap["agent_id"] = self._agent_id
            snap["last_reflect"] = self._last_reflect
            return snap

    def get_personality(self) -> dict[str, float]:
        """获取 OCEAN 人格配置"""
        with self._lock:
            return self._engine.get_personality()

    def format_prompt(self) -> str:
        """格式化 <emotion_state> Prompt XML"""
        with self._lock:
            return self._engine.format_prompt()

    def get_other_agents(self) -> list[dict[str, Any]]:
        """扫描同级 Agent 情绪状态 — 添加相对路径安全性保护"""
        # 注意：Rust 层现在已有基本路径安全性检测
        return self._engine.get_other_agents(".")

    @property
    def last_reflect(self) -> str | None:
        """最近一次反思文本"""
        return self._last_reflect

    # ═══════════════════════════════════════
    #  写操作 (加锁)
    # ═══════════════════════════════════════

    def apply_event(self, name: str, intensity: float = 1.0) -> dict[str, Any]:
        """应用情绪事件 — 自动触发 Evolve 记录 + Reflect 检查

        未知事件会被忽略并返回当前状态，不再崩溃。
        """
        with self._lock:
            try:
                result = self._engine.apply_event(name, intensity)
            except ValueError as e:
                if "Unknown event" in str(e):
                    # BUG-L1: 容错未知事件
                    result = self._engine.get_state()
                    result["agent_id"] = self._agent_id
                    result["last_reflect"] = self._last_reflect
                    return result
                raise
            self._maybe_save()

        # Evolve: 记录快照
        self._evolve_history.append(
            {
                "v": result["v"],
                "a": result["a"],
                "d": result["d"],
            }
        )
        if len(self._evolve_history) > 100:
            self._evolve_history = self._evolve_history[-100:]

        # Reflect: 自动检查
        self._reflect_counter += 1
        if self._reflect_counter >= self._reflect_interval:
            self._reflect_counter = 0
            self._last_reflect = self._generate_reflection(result)

        return result

    def modify(self, dimension: str, delta: float) -> dict[str, Any]:
        """直接修改单个维度"""
        with self._lock:
            result = self._engine.modify(dimension, delta)
            self._maybe_save()
            return result

    def reset(self, dimensions: list[str] | None = None) -> dict[str, Any]:
        """重置至 baseline"""
        with self._lock:
            result = self._engine.reset(dimensions)
            self._maybe_save()
            self._reflect_counter = 0
            self._evolve_history.clear()
            self._last_reflect = None
            return result

    def set_personality(self, trait_name: str, value: float) -> None:
        """设置人格特质"""
        with self._lock:
            self._engine.set_personality(trait_name, value)
            self._maybe_save()

    def save(self) -> None:
        """强制保存"""
        with self._lock:
            self._engine.save()
            self._last_save_ms = int(time.time() * 1000)

    def load(self) -> None:
        """从文件重新加载"""
        with self._lock:
            self._engine.load()

    # ═══════════════════════════════════════
    #  Reflect — 自我反思
    # ═══════════════════════════════════════

    def reflect(self) -> str:
        """强制触发一次反思，返回自我认知文本"""
        state = self.get_state()
        self._reflect_counter = 0
        self._last_reflect = self._generate_reflection(state)
        return self._last_reflect

    def _generate_reflection(self, state: dict[str, Any]) -> str:
        v, a, d = state["v"], state["a"], state["d"]
        tone = state.get("tone", "neutral")
        ruminations = state.get("active_ruminations", 0)

        parts = [f"[Reflect] V={v:+.2f} A={a:+.2f} D={d:+.2f} (tone: {tone})"]

        if ruminations > 0:
            parts.append(f"  Processing {ruminations} emotional aftereffect(s).")

        if v > 0.3:
            parts.append("  Overall positive. Recent interactions uplifting.")
        elif v < -0.3:
            parts.append("  Overall negative. Recent events distressing.")
        else:
            parts.append("  Emotionally balanced.")

        if a > 0.4:
            parts.append("  High energy — alert and responsive.")
        elif a < -0.3:
            parts.append("  Low energy — calm or fatigued.")

        if abs(d) > 0.3:
            word = "confident" if d > 0 else "uncertain"
            parts.append(f"  Sense of control: {word}.")

        return "\n".join(parts)

    # ═══════════════════════════════════════
    #  Dream — 梦境情绪碎片整理
    # ═══════════════════════════════════════

    def dream(self) -> dict[str, Any]:
        """
        执行一次"梦境" — 随机微弱 VAD 波动，模拟潜意识整理

        Returns:
            {"theme": str, "modifications": {"v": float, "a": float, "d": float}}
        """
        theme = random.choice(_DREAM_THEMES)  # noqa: S311

        for dim in ("v", "a", "d"):
            delta = theme[dim]
            if abs(delta) > 0.001:
                try:
                    self.modify(dim, delta)
                except Exception:
                    pass

        return {
            "theme": theme["theme"],
            "modifications": {"v": theme["v"], "a": theme["a"], "d": theme["d"]},
        }

    # ═══════════════════════════════════════
    #  Evolve — 人格漂移
    # ═══════════════════════════════════════

    def evolve(self) -> dict[str, float]:
        """
        基于累积历史统计执行一次人格微调

        Returns:
            {trait_name: delta_applied} 包含实际变动的特质，空 dict 表示数据不足
        """
        if len(self._evolve_history) < 10:
            return {}

        n = len(self._evolve_history)
        avg_v = sum(h["v"] for h in self._evolve_history) / n
        avg_a = sum(h["a"] for h in self._evolve_history) / n
        avg_d = sum(h["d"] for h in self._evolve_history) / n

        drifts = self._compute_drifts(avg_v, avg_a, avg_d)
        applied = self._apply_drifts(drifts)
        self._evolve_history.clear()
        return applied

    @staticmethod
    def _compute_drifts(avg_v: float, avg_a: float, avg_d: float) -> dict[str, float]:
        """根据平均 VAD 计算各特质的漂移量"""
        drifts: dict[str, float] = {}
        conditions = [
            (avg_v > 0.15, "positive"),
            (avg_v < -0.15, "negative"),
            (avg_a > 0.2, "high_arousal"),
            (avg_d > 0.2, "high_dominance"),
        ]
        for cond, key in conditions:
            if cond and key in _DRIFT_MAP:
                for trait, delta in _DRIFT_MAP[key].items():
                    drifts[trait] = drifts.get(trait, 0) + delta
        return drifts

    def _apply_drifts(self, drifts: dict[str, float]) -> dict[str, float]:
        """将漂移量限幅后应用到人格特质"""
        applied: dict[str, float] = {}
        for trait, delta in drifts.items():
            clamped = max(-_MAX_DRIFT_PER_CALL, min(_MAX_DRIFT_PER_CALL, delta))
            if abs(clamped) < 0.0001:
                continue
            p = self.get_personality()
            new_val = max(0.0, min(1.0, p[trait] + clamped))
            try:
                self.set_personality(trait, new_val)
                applied[trait] = clamped
            except Exception:
                pass
        return applied

    # ═══════════════════════════════════════
    #  Decay Loop — 后台自动衰减
    # ═══════════════════════════════════════

    def start_decay_loop(self, interval_sec: float = 60.0) -> None:
        """启动后台衰减循环，每 interval_sec 秒触发一次 get_state() 驱动按需衰减"""
        if self._decay_thread is not None and self._decay_thread.is_alive():
            return  # 已在运行

        self._decay_stop.clear()

        def _loop() -> None:
            while not self._decay_stop.wait(timeout=interval_sec):
                try:
                    self.get_state()
                except Exception:
                    pass

        self._decay_thread = threading.Thread(
            target=_loop, daemon=True, name="emotion-decay"
        )
        self._decay_thread.start()

    def stop_decay_loop(self) -> None:
        """停止后台衰减循环"""
        self._decay_stop.set()
        if self._decay_thread is not None:
            self._decay_thread.join(timeout=2.0)
            self._decay_thread = None

    # ── 内部 ────────────────────────────

    def _maybe_save(self) -> None:
        """防抖保存：距上次保存超过间隔才落盘"""
        now_ms = int(time.time() * 1000)
        if now_ms - self._last_save_ms >= self._save_interval_ms:
            self._engine.save()
            self._last_save_ms = now_ms


def get_engine(
    agent_id: str | None = None,
    config_dir: str | None = None,
    state_dir: str | None = None,
) -> EngineWrapper:
    """
    获取或创建全局单例引擎实例（按 agent_id 隔离）

    Args:
        agent_id:   Agent 标识符，默认从 EMOTION_AGENT_ID 读取
        config_dir: 配置目录，默认从 EMOTION_CONFIG_DIR 读取
        state_dir:  状态文件目录，默认从 EMOTION_STATE_DIR 读取

    Returns:
        线程安全的 EngineWrapper 实例（内置 Reflect/Dream/Evolve）
    """
    aid = agent_id or _DEFAULT_AGENT_ID
    cdir = config_dir or _DEFAULT_CONFIG_DIR
    sdir = state_dir or _DEFAULT_STATE_DIR

    if aid not in _instances:
        with _lock:
            if aid not in _instances:
                os.makedirs(sdir, exist_ok=True)
                state_path = os.path.join(sdir, f"{aid}.json")
                _instances[aid] = EngineWrapper(aid, cdir, state_path)

    return _instances[aid]
