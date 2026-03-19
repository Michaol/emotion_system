# BUG: `advance_ruminations` 被重复调用导致情绪值翻倍

## 问题描述

调用 `apply_event()` 后，再调用 `get_state()` 会导致情绪 VAD 值异常翻倍（或多次叠加），而不是保持稳定的状态。

### 复现步骤

```python
from emotion_engine._core import EmotionEngine

engine = EmotionEngine("config", "state/asuna.json", "test")
engine.reset()

# 应用一个事件
result = engine.apply_event("ancestor_warning", 1.0)
print(f"apply_event 返回：v={result['v']:.3f}, a={result['a']:.3f}, d={result['d']:.3f}")
# 输出：v=-0.400, a=0.500, d=-0.300 ✅ 正确

# 立即获取状态
state = engine.get_state()
print(f"get_state 返回：v={state['v']:.3f}, a={state['a']:.3f}, d={state['d']:.3f}")
# 输出：v=-0.800, a=1.000, d=-0.600 ❌ 翻倍了！

# 再次获取状态
state2 = engine.get_state()
print(f"get_state 再次返回：v={state2['v']:.3f}, a={state2['a']:.3f}, d={state2['d']:.3f}")
# 输出：v=-1.000, a=1.000, d=-0.750 ❌ 继续叠加！
```

## 根本原因

`advance_ruminations()` 函数在以下两个地方被调用：

1. **`apply_event()` 内部** (engine.rs:202):
   ```rust
   pub fn apply_event(&mut self, name: &str, intensity: f32) -> Result<EmotionSnapshot, ConfigError> {
       self.apply_decay_to_current();
       advance_ruminations(&mut self.state.current, &mut self.state.ruminations); // ← 第一次
       // ...
       self.state.current.apply_delta(scaled.v, scaled.a, scaled.d);
       // ...
   }
   ```

2. **`get_state()` 内部** (engine.rs:248):
   ```rust
   pub fn get_state(&mut self) -> EmotionSnapshot {
       self.apply_decay_to_current();
       advance_ruminations(&mut self.state.current, &mut self.state.ruminations); // ← 第二次！
       self.snapshot()
   }
   ```

### 问题机制

`advance_ruminations()` 的作用是将活跃的 rumination（情绪余波）增量应用到当前状态：

```rust
pub fn advance_ruminations(state: &mut VadState, entries: &mut Vec<RuminationEntry>) {
    entries.retain_mut(|entry| {
        if entry.remaining_rounds == 0 {
            return false;
        }
        let (dv, da, dd) = entry.current_contribution();
        state.apply_delta(dv, da, dd);  // ← 这里会修改状态！
        entry.remaining_rounds -= 1;
        entry.remaining_rounds > 0
    });
}
```

**问题**：每次调用 `advance_ruminations()` 都会将 rumination 的 delta **再次应用**到状态上。如果同一个事件触发了 rumination，那么：

1. `apply_event()` 调用时，rumination 被应用一次
2. `get_state()` 调用时，rumination **又被应用一次**
3. 每次调用 `get_state()` 都会重复应用，导致状态不断叠加

## 修复方案

### 方案 A：`get_state()` 不应该调用 `advance_ruminations()`（推荐）

`get_state()` 应该是**只读操作**，不应该修改状态。`advance_ruminations()` 的调用应该只在状态变更时进行（如 `apply_event()`、`modify()` 等）。

```rust
// 修改前
pub fn get_state(&mut self) -> EmotionSnapshot {
    self.apply_decay_to_current();
    advance_ruminations(&mut self.state.current, &mut self.state.ruminations); // ❌
    self.snapshot()
}

// 修改后
pub fn get_state(&self) -> EmotionSnapshot {
    // 不修改状态，只读快照
    self.snapshot()
}
```

但这样需要确保 `apply_decay_to_current()` 也是只读的，或者在 `snapshot()` 中处理衰减逻辑。

### 方案 B：添加标志位防止重复应用

在 `Engine` 结构体中添加一个标志位，标记本轮是否已经应用过 rumination：

```rust
pub struct Engine {
    // ...
    state: State,
    rumination_applied: bool,  // ← 新增
}

pub fn apply_event(&mut self, name: &str, intensity: f32) -> Result<EmotionSnapshot, ConfigError> {
    self.apply_decay_to_current();
    advance_ruminations(&mut self.state.current, &mut self.state.ruminations);
    self.rumination_applied = true;  // ← 标记
    // ...
}

pub fn get_state(&mut self) -> EmotionSnapshot {
    self.apply_decay_to_current();
    if !self.rumination_applied {  // ← 检查标志
        advance_ruminations(&mut self.state.current, &mut self.state.ruminations);
    }
    self.snapshot()
}
```

### 方案 C：将 rumination 应用与状态分离（彻底重构）

将 rumination 的增量计算与状态应用分离，`advance_ruminations()` 只负责更新 `remaining_rounds`，在 `snapshot()` 中动态计算当前应得的 rumination 增量。

## 影响范围

- 所有调用 `apply_event()` 后再调用 `get_state()` 的场景
- 后台衰减循环（如果存在周期性调用 `get_state()`）
- MCP 工具 `get_state`、`format_prompt` 等
- 可能导致情绪值快速达到极值（-1 或 1）且无法自然恢复

## 测试用例

修复后应通过以下测试：

```python
def test_no_double_apply():
    engine = EmotionEngine("config", "state/test.json", "test")
    engine.reset()
    
    # 应用事件
    result1 = engine.apply_event("ancestor_warning", 1.0)
    assert result1['v'] == -0.4  # 预期值
    
    # 多次 get_state 应该返回相同结果
    state1 = engine.get_state()
    state2 = engine.get_state()
    state3 = engine.get_state()
    
    assert state1['v'] == state2['v'] == state3['v']
    assert state1['a'] == state2['a'] == state3['a']
    assert state1['d'] == state2['d'] == state3['d']
    
    # 应该等于 apply_event 的返回值（考虑 decay 可忽略）
    assert abs(state1['v'] - (-0.4)) < 0.01
```

## 相关文件

- `core/src/engine.rs`: `apply_event()`, `get_state()`
- `core/src/rumination.rs`: `advance_ruminations()`
- `tests/test_double_apply.py`: 新增测试用例

## 优先级

**高** - 此 bug 导致情绪系统完全不可用，所有情绪值都会异常放大。
