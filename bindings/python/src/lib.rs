/*
 * Copyright (c) 2024-present Michaol (https://github.com/Michaol)
 * Part of Emotion Engine - VAD-based emotional simulation for AI Agents.
 * Licensed under CC BY-NC 4.0 (Attribution-NonCommercial 4.0 International).
 */

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::path::Path;

use emotion_core::engine::{EmotionSnapshot, Engine};
use emotion_core::multi_agent;
use emotion_core::persistence;
use emotion_core::prompt;

/// Python 可见的 EmotionEngine 类
#[pyclass]
pub struct EmotionEngine {
    inner: Engine,
}

/// 辅助：将 OceanProfile 转为 Python dict
fn personality_to_pydict<'py>(
    py: Python<'py>,
    p: &emotion_core::personality::OceanProfile,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    let dict = pyo3::types::PyDict::new(py);
    dict.set_item("openness", p.openness)?;
    dict.set_item("conscientiousness", p.conscientiousness)?;
    dict.set_item("extraversion", p.extraversion)?;
    dict.set_item("agreeableness", p.agreeableness)?;
    dict.set_item("neuroticism", p.neuroticism)?;
    Ok(dict)
}

/// 辅助：将 EmotionSnapshot 转为 Python dict
fn snapshot_to_pydict(py: Python<'_>, s: &EmotionSnapshot) -> PyResult<PyObject> {
    let dict = pyo3::types::PyDict::new(py);
    dict.set_item("v", s.v)?;
    dict.set_item("a", s.a)?;
    dict.set_item("d", s.d)?;
    dict.set_item("dominant_emotion", &s.dominant_emotion)?;
    dict.set_item("tone", &s.tone)?;
    dict.set_item("active_ruminations", s.active_ruminations)?;
    dict.set_item("personality", personality_to_pydict(py, &s.personality)?)?;

    // plutchik 分类结果
    let plutchik_dict = pyo3::types::PyDict::new(py);
    plutchik_dict.set_item("label", &s.plutchik.label)?;
    plutchik_dict.set_item("confidence", s.plutchik.confidence)?;
    dict.set_item("plutchik", plutchik_dict)?;

    Ok(dict.into())
}

#[pymethods]
impl EmotionEngine {
    /// 构造函数：加载配置和状态
    #[new]
    #[pyo3(signature = (config_path, state_path, agent_id=None))]
    fn new(config_path: &str, state_path: &str, agent_id: Option<&str>) -> PyResult<Self> {
        let inner = Engine::new(config_path, state_path, agent_id)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    /// 应用情绪事件
    #[pyo3(signature = (name, intensity=1.0))]
    fn apply_event(&mut self, py: Python<'_>, name: &str, intensity: f32) -> PyResult<PyObject> {
        let snap = self
            .inner
            .apply_event(name, intensity)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        snapshot_to_pydict(py, &snap)
    }

    /// 获取当前情绪状态
    fn get_state(&mut self, py: Python<'_>) -> PyResult<PyObject> {
        let snap = self.inner.get_state();
        snapshot_to_pydict(py, &snap)
    }

    /// 直接修改单个维度
    fn modify(&mut self, py: Python<'_>, dimension: &str, delta: f32) -> PyResult<PyObject> {
        let snap = self
            .inner
            .modify(dimension, delta)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        snapshot_to_pydict(py, &snap)
    }

    /// 重置至 baseline
    #[pyo3(signature = (dimensions=None))]
    fn reset(&mut self, py: Python<'_>, dimensions: Option<Vec<String>>) -> PyResult<PyObject> {
        let snap = match dimensions {
            Some(ref dims) => self.inner.reset(Some(dims)),
            None => self.inner.reset(None),
        };
        snapshot_to_pydict(py, &snap)
    }

    /// 设置人格特质
    fn set_personality(&mut self, trait_name: &str, value: f32) -> PyResult<()> {
        self.inner
            .set_personality(trait_name, value)
            .map_err(|e| PyValueError::new_err(e))
    }

    /// 获取人格配置
    fn get_personality(&self, py: Python<'_>) -> PyResult<PyObject> {
        let p = self.inner.get_personality();
        Ok(personality_to_pydict(py, p)?.into())
    }

    /// 扫描其他 Agent 的情绪状态（增加安全性检查）
    fn get_other_agents(&self, py: Python<'_>, scan_dir: &str) -> PyResult<PyObject> {
        let scan_path = Path::new(scan_dir);
        // 如果扫描目录不是相对于状态文件夹的路径，或包含 .. 尝试跳转，则只在原地扫描
        if scan_path.is_absolute() || scan_dir.contains("..") {
            return Err(PyValueError::new_err(
                "Invalid scan directory: path must be relative and safe",
            ));
        }

        let agents = multi_agent::scan_other_agents(scan_dir, &self.inner.state_path);
        let list = pyo3::types::PyList::empty(py);
        for a in &agents {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("agent_id", &a.agent_id)?;
            dict.set_item("v", a.v)?;
            dict.set_item("a", a.a)?;
            dict.set_item("d", a.d)?;
            dict.set_item("last_updated_ms", a.last_updated_ms)?;
            list.append(dict)?;
        }
        Ok(list.into())
    }

    /// 格式化 Prompt XML
    fn format_prompt(&mut self) -> String {
        prompt::format_emotion_prompt(&mut self.inner)
    }

    /// 保存状态到文件
    fn save(&mut self) -> PyResult<()> {
        persistence::save_state(&mut self.inner).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// 从文件加载状态
    fn load(&mut self) -> PyResult<()> {
        persistence::load_state(&mut self.inner).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// 检索与当前情绪相关的记忆
    #[pyo3(signature = (top_k=3))]
    fn get_memories(&self, py: Python<'_>, top_k: usize) -> PyResult<PyObject> {
        let memories = self.inner.get_memories(top_k);
        let list = pyo3::types::PyList::empty(py);
        for m in &memories {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("event_name", &m.event_name)?;
            dict.set_item("salience", m.salience)?;
            dict.set_item("retention", m.retention)?;
            dict.set_item("recall_count", m.recall_count)?;
            dict.set_item("effective_strength", m.effective_strength())?;
            dict.set_item("timestamp_ms", m.timestamp_ms)?;
            list.append(dict)?;
        }
        Ok(list.into())
    }
}

/// Python 模块
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EmotionEngine>()?;
    Ok(())
}
