"""
Brain Loop — 静默周期性情绪事件调度器

根据时间段自动注入低强度后台情绪事件:
  - 深夜 (NightDream): 微负 A/D，模拟沉淀、内观
  - 傍晚 (EveningReflect): 微负 A，模拟日终疲劳/反思
  - 清晨 (MorningWake): 微正 V/A，模拟精力恢复

用法:
    from wrappers.python.brain_loop import BrainLoop, start_brain_loop

    loop = start_brain_loop(engine)   # 启动后台守护线程
    loop.stop()                        # 停止
"""
from __future__ import annotations

import threading
import time
from datetime import datetime
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .engine import EngineWrapper


# ── 时段事件定义 ──────────────────────────

SCHEDULED_EVENTS = [
    {
        "name": "NightDream",
        "hours": range(0, 6),      # 00:00 ~ 05:59
        "event": "calm",
        "intensity": 0.15,
        "fallback_modify": {"a": -0.05, "d": -0.03},
    },
    {
        "name": "EveningReflect",
        "hours": range(20, 24),    # 20:00 ~ 23:59
        "event": "nostalgia",
        "intensity": 0.10,
        "fallback_modify": {"a": -0.04, "v": -0.02},
    },
    {
        "name": "MorningWake",
        "hours": range(6, 10),     # 06:00 ~ 09:59
        "event": "contentment",
        "intensity": 0.12,
        "fallback_modify": {"v": 0.03, "a": 0.02},
    },
]

# 默认调度间隔 (秒)
DEFAULT_INTERVAL = 1800  # 30 分钟


class BrainLoop:
    """后台守护线程：周期性注入时段情绪事件"""

    def __init__(
        self,
        engine: EngineWrapper,
        interval_seconds: int = DEFAULT_INTERVAL,
    ) -> None:
        self._engine = engine
        self._interval = interval_seconds
        self._running = False
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        """启动后台调度"""
        if self._running:
            return
        self._running = True
        self._thread = threading.Thread(target=self._loop, daemon=True, name="BrainLoop")
        self._thread.start()

    def stop(self) -> None:
        """停止后台调度"""
        self._running = False
        if self._thread:
            self._thread.join(timeout=5)
            self._thread = None

    @property
    def is_running(self) -> bool:
        return self._running

    def tick(self) -> str | None:
        """
        手动执行一次调度 tick (也被后台线程调用)

        Returns:
            触发的事件名，或 None
        """
        hour = datetime.now().hour
        triggered = self._try_scheduled_event(hour)

        # 夜间自动触发 dream
        if triggered == "NightDream":
            self._engine.dream()

        # 每次 tick 尝试 evolve (仅在积累足够历史时生效)
        self._engine.evolve()

        return triggered

    def _try_scheduled_event(self, hour: int) -> str | None:
        """尝试匹配并执行时段事件"""
        for sched in SCHEDULED_EVENTS:
            if hour not in sched["hours"]:
                continue
            try:
                self._engine.apply_event(sched["event"], sched["intensity"])
            except ValueError:
                for dim, delta in sched["fallback_modify"].items():
                    try:
                        self._engine.modify(dim, delta)
                    except Exception:
                        pass
            return sched["name"]
        return None

    def _loop(self) -> None:
        """后台循环"""
        while self._running:
            self.tick()
            time.sleep(self._interval)


def start_brain_loop(
    engine: EngineWrapper,
    interval_seconds: int = DEFAULT_INTERVAL,
) -> BrainLoop:
    """便捷函数：创建并启动 BrainLoop"""
    loop = BrainLoop(engine, interval_seconds)
    loop.start()
    return loop
