const API_BASE = 'http://localhost:8000';

async function refresh() {
    try {
        const response = await fetch(`${API_BASE}/state`);
        const data = await response.json();
        updateUI(data);
    } catch (e) {
        console.error("Failed to fetch state:", e);
    }
}

function updateUI(data) {
    const { snapshot, personality, prompt, reflect } = data;

    // VAD Gauges
    updateValue('gauge-v', snapshot.v);
    updateValue('gauge-a', snapshot.a);
    updateValue('gauge-d', snapshot.d);
    document.getElementById('tone-text').textContent = snapshot.tone || 'Neutral';

    // OCEAN Bars
    updateBar('trait-o', personality.openness);
    updateBar('trait-c', personality.conscientiousness);
    updateBar('trait-e', personality.extraversion);
    updateBar('trait-a', personality.agreeableness);
    updateBar('trait-n', personality.neuroticism);

    // Agent ID
    document.getElementById('agent-id-display').textContent = snapshot.agent_id || 'dashboard_test';

    // Prompt Preview
    document.getElementById('prompt-content').textContent = prompt;

    // Reflection
    document.getElementById('reflection-text').textContent = reflect || "No reflection yet...";
}

function updateValue(id, val) {
    const el = document.getElementById(id);
    const v = val.toFixed(2);
    el.querySelector('.value').textContent = (v > 0 ? '+' : '') + v;
    
    // Simple color shift based on value
    const saturation = Math.min(Math.abs(val) * 100, 100);
    const hue = val > 0 ? 200 : 0; // blue-ish vs red-ish
    el.style.color = `hsl(${hue}, ${saturation}%, 60%)`;
    el.style.borderColor = `hsla(${hue}, ${saturation}%, 60%, 0.3)`;
}

function updateBar(id, val) {
    const el = document.getElementById(id);
    el.style.width = `${val * 100}%`;
}

// Actions
document.getElementById('btn-apply').onclick = async () => {
    const name = document.getElementById('event-name').value;
    const intensity = Number.parseFloat(document.getElementById('event-intensity').value);
    
    if (!name) return;

    const response = await fetch(`${API_BASE}/apply`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, intensity })
    });
    updateUI(await response.json());
};

document.getElementById('btn-reset').onclick = async () => {
    const response = await fetch(`${API_BASE}/reset`, { method: 'POST' });
    updateUI(await response.json());
};

document.getElementById('btn-reflect').onclick = async () => {
    const response = await fetch(`${API_BASE}/reflect`, { method: 'POST' });
    updateUI(await response.json());
};

document.getElementById('btn-dream').onclick = async () => {
    const response = await fetch(`${API_BASE}/dream`, { method: 'POST' });
    const data = await response.json();
    updateUI(data);
    alert(`Dreaming about: ${data.dream.theme}`);
};

document.getElementById('btn-evolve').onclick = async () => {
    const response = await fetch(`${API_BASE}/evolve`, { method: 'POST' });
    const data = await response.json();
    updateUI(data);
    if (Object.keys(data.drift).length > 0) {
        alert("Personality evolved: " + JSON.stringify(data.drift));
    } else {
        alert("Insufficient history for evolution (need 10+ events).");
    }
};

// Start Polling
setInterval(refresh, 2000);
await refresh();
