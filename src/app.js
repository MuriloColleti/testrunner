// ─────────────────────────────────────────────────────────────────────────────
//  Znap Test Runner — app.js
//  Uses Tauri v2 global API (withGlobalTauri: true)
// ─────────────────────────────────────────────────────────────────────────────

const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

// ── State ─────────────────────────────────────────────────────────────────────

const state = {
  suites:   [],
  runs:     new Map(), // suiteId → { status, startTime, duration, passCount, failCount }
  timers:   new Map(), // suiteId → intervalId
  activeTab: null,
};

// ── DOM refs ──────────────────────────────────────────────────────────────────

const $suiteList  = document.getElementById('suiteList');
const $tabsBar    = document.getElementById('tabsBar');
const $tabsList   = document.getElementById('tabsList');
const $content    = document.getElementById('content');
const $emptyState = document.getElementById('emptyState');

// ── Bootstrap ─────────────────────────────────────────────────────────────────

async function init() {
  await registerListeners();
  await loadSuites();
}

async function loadSuites() {
  try {
    state.suites = await invoke('get_suites');
    renderSidebar();
  } catch (e) {
    $suiteList.innerHTML = `<div class="sidebar-loading" style="color:#f85149">Erro: ${e}</div>`;
  }
}

// ── Tauri event listeners ─────────────────────────────────────────────────────

async function registerListeners() {
  await listen('suite-started', ({ payload }) => {
    const run = state.runs.get(payload.suiteId);
    if (run) { run.status = 'running'; run.startTime = Date.now(); }
    startTimer(payload.suiteId);
    setPanelStatus(payload.suiteId, 'running');
    setSidebarDot(payload.suiteId, 'running');
    setTabDot(payload.suiteId, 'running');
  });

  await listen('suite-output', ({ payload }) => {
    const run = state.runs.get(payload.suiteId);
    if (!run) return;
    if (/^\s*[✓✔]/.test(payload.line)) run.passCount++;
    if (/^\s*[✗×✘]/.test(payload.line)) run.failCount++;
    appendLine(payload.suiteId, payload.line);
    updateStats(payload.suiteId);
  });

  await listen('suite-done', ({ payload }) => {
    const run = state.runs.get(payload.suiteId);
    if (run) { run.status = payload.status; run.duration = payload.duration; }
    stopTimer(payload.suiteId);
    setPanelStatus(payload.suiteId, payload.status, payload.duration);
    setSidebarDot(payload.suiteId, payload.status);
    setTabDot(payload.suiteId, payload.status);
    setRunBtn(payload.suiteId, false);
  });
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

function renderSidebar() {
  const groups = {};
  for (const s of state.suites) {
    (groups[s.system] ||= []).push(s);
  }

  $suiteList.innerHTML = Object.entries(groups).map(([sys, list]) => `
    <div class="system-group">
      <div class="system-label">${sys}</div>
      ${list.map((s) => `
        <div class="suite-row" id="row-${s.id}">
          <span class="suite-dot" id="dot-${s.id}"></span>
          <div class="suite-info">
            <span class="suite-name">${s.name}</span>
            <span class="tag ${s.tag === 'API' ? 'tag-api' : 'tag-e2e'}">${s.tag}</span>
          </div>
          <button class="run-btn" id="runbtn-${s.id}" title="Executar">▶</button>
        </div>
      `).join('')}
    </div>
  `).join('');

  // Attach click handlers after rendering
  for (const s of state.suites) {
    document.getElementById(`runbtn-${s.id}`)
      .addEventListener('click', () => handleRunBtn(s.id));
  }
}

// ── Run / Stop ────────────────────────────────────────────────────────────────

function handleRunBtn(suiteId) {
  const run = state.runs.get(suiteId);
  if (run?.status === 'running') {
    invoke('stop_suite', { suiteId });
  } else {
    startRun(suiteId);
  }
}

function startRun(suiteId) {
  state.runs.set(suiteId, {
    status: 'running',
    startTime: Date.now(),
    duration: 0,
    passCount: 0,
    failCount: 0,
  });

  ensurePanel(suiteId);
  clearTerminal(suiteId);
  activateTab(suiteId);
  setRunBtn(suiteId, true);
  setSidebarDot(suiteId, 'running');

  invoke('run_suite', { suiteId }).catch((e) => {
    appendLine(suiteId, `Erro: ${e}`);
  });
}

// ── Tabs ──────────────────────────────────────────────────────────────────────

function ensurePanel(suiteId) {
  if (!document.getElementById(`panel-${suiteId}`)) {
    createPanel(suiteId);
    createTab(suiteId);
    $tabsBar.style.display = 'flex';
  }
}

function createTab(suiteId) {
  const s = state.suites.find((x) => x.id === suiteId);
  const tab = document.createElement('div');
  tab.className = 'tab';
  tab.id = `tab-${suiteId}`;
  tab.innerHTML = `
    <span class="tab-dot" id="tabdot-${suiteId}"></span>
    <span class="tab-label">${s.system} · ${s.name}</span>
    <span class="tab-close" id="close-${suiteId}">×</span>
  `;
  tab.addEventListener('click', () => activateTab(suiteId));
  document.getElementById(`close-${suiteId}`, tab).addEventListener('click', (e) => {
    e.stopPropagation();
    closeTab(suiteId);
  });
  $tabsList.appendChild(tab);
}

function activateTab(suiteId) {
  state.activeTab = suiteId;

  document.querySelectorAll('.tab').forEach((t) => t.classList.remove('active'));
  document.querySelectorAll('.panel').forEach((p) => p.classList.remove('active'));
  document.querySelectorAll('.suite-row').forEach((r) => r.classList.remove('row-active'));

  document.getElementById(`tab-${suiteId}`)?.classList.add('active');
  document.getElementById(`panel-${suiteId}`)?.classList.add('active');
  document.getElementById(`row-${suiteId}`)?.classList.add('row-active');

  $emptyState.style.display = 'none';
}

function closeTab(suiteId) {
  const run = state.runs.get(suiteId);
  if (run?.status === 'running') invoke('stop_suite', { suiteId });

  document.getElementById(`tab-${suiteId}`)?.remove();
  document.getElementById(`panel-${suiteId}`)?.remove();
  document.getElementById(`row-${suiteId}`)?.classList.remove('row-active');
  stopTimer(suiteId);
  state.runs.delete(suiteId);
  setSidebarDot(suiteId, 'idle');
  setRunBtn(suiteId, false);

  const remaining = document.querySelectorAll('.tab');
  if (remaining.length === 0) {
    $tabsBar.style.display = 'none';
    $emptyState.style.display = 'flex';
    state.activeTab = null;
  } else {
    const lastId = remaining[remaining.length - 1].id.replace('tab-', '');
    activateTab(lastId);
  }
}

// ── Panel ─────────────────────────────────────────────────────────────────────

function createPanel(suiteId) {
  const s = state.suites.find((x) => x.id === suiteId);
  const panel = document.createElement('div');
  panel.className = 'panel';
  panel.id = `panel-${suiteId}`;

  // Header
  const header = document.createElement('div');
  header.className = 'panel-header';
  header.innerHTML = `
    <div class="panel-title">
      <span class="panel-system">${s.system}</span>
      <span class="tag ${s.tag === 'API' ? 'tag-api' : 'tag-e2e'}">${s.tag}</span>
      <span class="panel-name">${s.name}</span>
    </div>
    <span class="status-badge" id="badge-${suiteId}">Aguardando</span>
    <span class="timer" id="timer-${suiteId}">0.0s</span>
  `;

  const rerunBtn = document.createElement('button');
  rerunBtn.className = 'act-btn';
  rerunBtn.id = `rerun-${suiteId}`;
  rerunBtn.textContent = '↺ Reexecutar';
  rerunBtn.style.display = 'none';
  rerunBtn.addEventListener('click', () => startRun(suiteId));

  const stopBtn = document.createElement('button');
  stopBtn.className = 'act-btn act-danger';
  stopBtn.id = `stop-${suiteId}`;
  stopBtn.textContent = '■ Parar';
  stopBtn.addEventListener('click', () => invoke('stop_suite', { suiteId }));

  header.appendChild(rerunBtn);
  header.appendChild(stopBtn);

  // Terminal
  const term = document.createElement('div');
  term.className = 'terminal';
  term.id = `terminal-${suiteId}`;
  term.innerHTML = '<span class="cursor"></span>';

  // Stats bar
  const stats = document.createElement('div');
  stats.className = 'stats-bar';
  stats.id = `stats-${suiteId}`;
  stats.innerHTML = `
    <span class="stat">Testes: <b id="total-${suiteId}">—</b></span>
    <span class="stat passed-stat">Passou: <b id="pass-${suiteId}">0</b></span>
    <span class="stat failed-stat">Falhou: <b id="fail-${suiteId}">0</b></span>
  `;

  panel.appendChild(header);
  panel.appendChild(term);
  panel.appendChild(stats);
  $content.appendChild(panel);
}

function clearTerminal(suiteId) {
  const term = document.getElementById(`terminal-${suiteId}`);
  if (term) term.innerHTML = '<span class="cursor"></span>';
  const run = state.runs.get(suiteId);
  if (run) { run.passCount = 0; run.failCount = 0; }
  updateStats(suiteId);
}

function appendLine(suiteId, line) {
  const term = document.getElementById(`terminal-${suiteId}`);
  if (!term) return;

  const cursor = term.querySelector('.cursor');
  if (cursor) cursor.remove();

  const div = document.createElement('div');
  div.className = 'tline ' + classifyLine(line);
  div.textContent = line;
  term.appendChild(div);

  const newCursor = document.createElement('span');
  newCursor.className = 'cursor';
  term.appendChild(newCursor);

  term.scrollTop = term.scrollHeight;
}

function classifyLine(line) {
  const t = line.trim();
  if (/^[✓✔]/.test(t))                               return 'l-pass';
  if (/^[✗×✘]/.test(t))                               return 'l-fail';
  if (/^●/.test(t))                                   return 'l-running';
  if (/^\d+ passed/.test(t) && !/failed/.test(t))     return 'l-sum-pass';
  if (/\d+ failed/.test(t))                           return 'l-sum-fail';
  if (/Error:/i.test(t) && !/^\s{2,}at /.test(line))  return 'l-error';
  if (/^\s{2,}at /.test(line))                        return 'l-stack';
  if (/^Running \d+/.test(t))                         return 'l-info';
  return 'l-default';
}

// ── Status helpers ────────────────────────────────────────────────────────────

function setRunBtn(suiteId, isRunning) {
  const btn = document.getElementById(`runbtn-${suiteId}`);
  if (btn) {
    btn.className = 'run-btn' + (isRunning ? ' running' : '');
    btn.textContent = isRunning ? '◼' : '▶';
    btn.title = isRunning ? 'Parar' : 'Executar';
  }
  const stopEl  = document.getElementById(`stop-${suiteId}`);
  const rerunEl = document.getElementById(`rerun-${suiteId}`);
  if (stopEl)  stopEl.style.display  = isRunning ? 'flex' : 'none';
  if (rerunEl) rerunEl.style.display = isRunning ? 'none' : 'flex';
}

function setSidebarDot(suiteId, status) {
  const dot = document.getElementById(`dot-${suiteId}`);
  if (dot) dot.className = `suite-dot ${status}`;
}

function setTabDot(suiteId, status) {
  const dot = document.getElementById(`tabdot-${suiteId}`);
  if (dot) dot.className = `tab-dot ${status}`;
}

function setPanelStatus(suiteId, status, duration) {
  const badge = document.getElementById(`badge-${suiteId}`);
  if (badge) {
    const labels = { running: 'Executando', passed: 'Passou', failed: 'Falhou', stopped: 'Parado' };
    badge.textContent = labels[status] || status;
    badge.className = `status-badge ${status}`;
  }

  if (duration != null && status !== 'running') {
    const timerEl = document.getElementById(`timer-${suiteId}`);
    if (timerEl) timerEl.textContent = fmtDuration(duration);
    // Remove blinking cursor when done
    document.querySelector(`#terminal-${suiteId} .cursor`)?.remove();
  }

  setRunBtn(suiteId, status === 'running');
}

function updateStats(suiteId) {
  const run = state.runs.get(suiteId);
  if (!run) return;
  const total = run.passCount + run.failCount;
  const setText = (id, val) => {
    const el = document.getElementById(id);
    if (el) el.textContent = val;
  };
  setText(`pass-${suiteId}`,  run.passCount);
  setText(`fail-${suiteId}`,  run.failCount);
  setText(`total-${suiteId}`, total > 0 ? total : '—');
}

// ── Timer ─────────────────────────────────────────────────────────────────────

function startTimer(suiteId) {
  stopTimer(suiteId);
  const id = setInterval(() => {
    const run = state.runs.get(suiteId);
    if (!run) return stopTimer(suiteId);
    const el = document.getElementById(`timer-${suiteId}`);
    if (el) el.textContent = fmtDuration(Date.now() - run.startTime);
  }, 100);
  state.timers.set(suiteId, id);
}

function stopTimer(suiteId) {
  const id = state.timers.get(suiteId);
  if (id != null) { clearInterval(id); state.timers.delete(suiteId); }
}

function fmtDuration(ms) {
  if (ms < 60000) return (ms / 1000).toFixed(1) + 's';
  return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
}

// ── Start ─────────────────────────────────────────────────────────────────────

init();
