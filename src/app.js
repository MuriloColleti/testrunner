// ─────────────────────────────────────────────────────────────────────────────
//  ReportTest — app.js
// ─────────────────────────────────────────────────────────────────────────────

let invoke, listen;

// ── State ─────────────────────────────────────────────────────────────────────

const state = {
  projects:         [],
  runs:             new Map(),   // suiteId → { status, startTime, duration, passCount, failCount, projectId, projectName, projectPath, suiteName, suiteTag, suiteArgs }
  timers:           new Map(),
  activeTab:        null,
  editingProjectId: null,
  pendingSuites:    [],
};

// ── DOM refs ──────────────────────────────────────────────────────────────────

const $suiteList     = document.getElementById('suiteList');
const $tabsBar       = document.getElementById('tabsBar');
const $tabsList      = document.getElementById('tabsList');
const $content       = document.getElementById('content');
const $emptyState    = document.getElementById('emptyState');
const $emptyTitle    = document.getElementById('emptyTitle');
const $emptySub      = document.getElementById('emptySub');
const $emptyAddBtn   = document.getElementById('emptyAddBtn');
const $configModal   = document.getElementById('configModal');
const $modalTitle    = document.getElementById('modalTitle');
const $inputName     = document.getElementById('inputName');
const $inputPath     = document.getElementById('inputPath');
const $verifyBtn     = document.getElementById('verifyBtn');
const $verifyResult  = document.getElementById('verifyResult');
const $modalSaveBtn  = document.getElementById('modalSaveBtn');
const $modalCancel   = document.getElementById('modalCancelBtn');
const $modalClose    = document.getElementById('modalCloseBtn');
const $addProjectBtn = document.getElementById('addProjectBtn');
const $browseBtn     = document.getElementById('browseBtn');

// ── Boot ──────────────────────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', async () => {
  const tauri = window.__TAURI__;
  if (!tauri) {
    $suiteList.innerHTML = '<div class="sidebar-empty-msg" style="color:#f85149">Erro: Tauri API indisponível.<br>Abra via <code>npm run dev</code>.</div>';
    return;
  }

  invoke = tauri.core.invoke;
  listen = tauri.event.listen;

  bindUI();
  await registerTauriListeners();
  await loadProjects();
});

// ── Projects ──────────────────────────────────────────────────────────────────

async function loadProjects() {
  try {
    state.projects = await invoke('get_projects');
    renderSidebar();
  } catch (e) {
    $suiteList.innerHTML = `<div class="sidebar-empty-msg" style="color:#f85149">Erro: ${e}</div>`;
  }
}

// ── UI bindings ───────────────────────────────────────────────────────────────

function bindUI() {
  $addProjectBtn.addEventListener('click', () => openModal());
  $emptyAddBtn.addEventListener('click',   () => openModal());
  $verifyBtn.addEventListener('click',     verifyPath);
  $modalSaveBtn.addEventListener('click',  saveProject);
  $modalCancel.addEventListener('click',   closeModal);
  $modalClose.addEventListener('click',    closeModal);
  $configModal.addEventListener('click',   (e) => { if (e.target === $configModal) closeModal(); });
  $inputPath.addEventListener('keydown',   (e) => { if (e.key === 'Enter') verifyPath(); });
  $browseBtn.addEventListener('click',     browsePath);
  $inputName.addEventListener('keydown',   (e) => { if (e.key === 'Enter') $inputPath.focus(); });
}

// ── Tauri event listeners ─────────────────────────────────────────────────────

async function registerTauriListeners() {
  await listen('suite-started', ({ payload }) => {
    const run = state.runs.get(payload.suiteId);
    // resync startTime from backend in case there was a delay
    if (run) { run.status = 'running'; run.startTime = Date.now(); }
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

  // Refresh history when a run completes and is saved to DB
  await listen('runs-updated', () => {
    // Could refresh a history panel here in a future iteration
  });
}

// ── Config Modal ──────────────────────────────────────────────────────────────

function openModal(projectId = null) {
  state.editingProjectId = projectId;
  state.pendingSuites    = [];
  $verifyResult.style.display = 'none';
  $modalSaveBtn.disabled = true;

  if (projectId) {
    const proj = state.projects.find((p) => p.id === projectId);
    $modalTitle.textContent  = 'Editar Projeto';
    $inputName.value = proj.name;
    $inputPath.value = proj.path;
    state.pendingSuites = proj.suites;
    $modalSaveBtn.disabled = false;
    showVerifyOk(proj.suites.length);
  } else {
    $modalTitle.textContent = 'Adicionar Projeto';
    $inputName.value = '';
    $inputPath.value = '';
  }

  $configModal.classList.add('open');
  setTimeout(() => $inputName.focus(), 50);
}

function closeModal() {
  $configModal.classList.remove('open');
  state.editingProjectId = null;
  state.pendingSuites    = [];
}

async function browsePath() {
  $browseBtn.disabled = true;
  try {
    const folder = await invoke('pick_folder');
    if (folder) {
      $inputPath.value = folder;
      // Auto-fill name from folder name if empty
      if (!$inputName.value.trim()) {
        const parts = folder.replace(/\\/g, '/').split('/');
        $inputName.value = parts[parts.length - 1] || '';
      }
      // Auto-verify after picking
      await verifyPath();
    }
  } catch (e) {
    console.error('pick_folder error:', e);
  } finally {
    $browseBtn.disabled = false;
  }
}

async function verifyPath() {
  const path = $inputPath.value.trim();
  if (!path) { showVerifyError('Informe o caminho do projeto.'); return; }

  $verifyBtn.textContent = 'Verificando...';
  $verifyBtn.disabled    = true;
  $verifyResult.style.display = 'none';
  $modalSaveBtn.disabled = true;

  try {
    const suites = await invoke('scan_project', { path });
    state.pendingSuites = suites;
    showVerifyOk(suites.length);
    $modalSaveBtn.disabled = false;
  } catch (err) {
    showVerifyError(String(err));
    state.pendingSuites = [];
  } finally {
    $verifyBtn.innerHTML = `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14"><circle cx="6.5" cy="6.5" r="4"/><path d="M11 11l3 3"/></svg> Verificar Caminho`;
    $verifyBtn.disabled = false;
  }
}

function showVerifyOk(count) {
  $verifyResult.className = 'verify-result ok';
  $verifyResult.textContent = `✓ ${count} suite(s) encontrada(s).`;
  $verifyResult.style.display = 'block';
}

function showVerifyError(msg) {
  $verifyResult.className = 'verify-result error';
  $verifyResult.textContent = `✗ ${msg}`;
  $verifyResult.style.display = 'block';
}

async function saveProject() {
  const name = $inputName.value.trim();
  const path = $inputPath.value.trim();
  if (!name) { $inputName.focus(); return; }
  if (!path) { $inputPath.focus(); return; }
  if (!state.pendingSuites.length) { await verifyPath(); return; }

  // Reuse existing ID if a project with the same path already exists
  const byPath = state.projects.find((p) => p.path === path);
  const project = {
    id:     state.editingProjectId || byPath?.id || crypto.randomUUID(),
    name,
    path,
    suites: state.pendingSuites,
  };

  try {
    await invoke('save_project', { project });
    await loadProjects();
    closeModal();
  } catch (e) {
    showVerifyError(`Erro ao salvar: ${e}`);
  }
}

async function deleteProject(projectId) {
  if (!confirm('Remover este projeto e todo o seu histórico?')) return;
  try {
    await invoke('delete_project', { projectId });
    await loadProjects();
  } catch (e) {
    alert(`Erro: ${e}`);
  }
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

function renderSidebar() {
  if (state.projects.length === 0) {
    $suiteList.innerHTML = '<div class="sidebar-empty-msg">Nenhum projeto.<br>Clique em <b>+</b> para adicionar.</div>';
    $emptyTitle.textContent = 'Nenhum projeto configurado';
    $emptySub.textContent   = 'Adicione um projeto para começar';
    $emptyAddBtn.style.display = 'flex';
    return;
  }

  $emptyAddBtn.style.display = 'none';

  $suiteList.innerHTML = state.projects.map((proj) => `
    <div class="project-section" id="project-${proj.id}">
      <div class="project-header">
        <span class="project-name">${proj.name}</span>
        <button class="project-edit-btn" data-edit="${proj.id}" title="Editar">✎</button>
      </div>
      ${proj.suites.map((s) => `
        <div class="suite-row" id="row-${s.id}">
          <span class="suite-dot" id="dot-${s.id}"></span>
          <div class="suite-info">
            <span class="suite-name">${s.system} · ${s.name}</span>
            <span class="tag ${s.tag === 'API' ? 'tag-api' : s.tag === 'Unit' ? 'tag-unit' : 'tag-e2e'}">${s.tag}</span>
          </div>
          <button class="run-btn" id="runbtn-${s.id}" data-suite="${s.id}" data-project="${proj.id}" title="Executar">▶</button>
        </div>
      `).join('')}
    </div>
  `).join('');

  $suiteList.querySelectorAll('[data-edit]').forEach((btn) => {
    btn.addEventListener('click', () => openModal(btn.dataset.edit));
  });

  $suiteList.querySelectorAll('[data-suite]').forEach((btn) => {
    btn.addEventListener('click', () => handleRunBtn(btn.dataset.suite, btn.dataset.project));
  });
}

// ── Run / Stop ────────────────────────────────────────────────────────────────

function handleRunBtn(suiteId, projectId) {
  const run = state.runs.get(suiteId);
  if (run?.status === 'running') {
    invoke('stop_suite', { suiteId });
  } else {
    startRun(suiteId, projectId);
  }
}

function startRun(suiteId, projectId) {
  const proj  = state.projects.find((p) => p.id === projectId);
  const suite = proj?.suites.find((s) => s.id === suiteId);
  if (!proj || !suite) return;

  state.runs.set(suiteId, {
    status:     'running',
    startTime:  Date.now(),
    duration:   0,
    passCount:  0,
    failCount:  0,
    totalCount: null,
    projectId:    proj.id,
    projectName:  proj.name,
    projectPath:  proj.path,
    suiteName:    suite.name,
    suiteTag:     suite.tag,
    suiteCommand: suite.command,
    suiteCwd:     suite.cwd,
    suiteArgs:    suite.args,
  });

  ensurePanel(suiteId, suite, proj);
  clearTerminal(suiteId);
  activateTab(suiteId);
  setRunBtn(suiteId, true);
  setSidebarDot(suiteId, 'running');
  setPanelStatus(suiteId, 'running');
  startTimer(suiteId);

  invoke('run_suite', {
    projectId:    proj.id,
    projectName:  proj.name,
    projectPath:  proj.path,
    suiteId,
    suiteName:    suite.name,
    suiteTag:     suite.tag,
    suiteCommand: suite.command,
    suiteCwd:     suite.cwd,
    suiteArgs:    suite.args,
  }).catch((e) => appendLine(suiteId, `Erro: ${e}`));
}

// ── Tabs ──────────────────────────────────────────────────────────────────────

function ensurePanel(suiteId, suite, proj) {
  if (!document.getElementById(`panel-${suiteId}`)) {
    createPanel(suiteId, suite, proj);
    createTab(suiteId, suite, proj);
    $tabsBar.style.display = 'flex';
  }
}

function createTab(suiteId, suite, proj) {
  const tab = document.createElement('div');
  tab.className = 'tab';
  tab.id = `tab-${suiteId}`;
  tab.innerHTML = `
    <span class="tab-dot" id="tabdot-${suiteId}"></span>
    <span>${proj.name} · ${suite.name}</span>
    <span class="tab-close" id="close-${suiteId}">×</span>
  `;
  tab.addEventListener('click', () => activateTab(suiteId));
  tab.querySelector(`#close-${suiteId}`).addEventListener('click', (e) => {
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
  setSidebarDot(suiteId, '');
  setRunBtn(suiteId, false);

  const remaining = document.querySelectorAll('.tab');
  if (remaining.length === 0) {
    $tabsBar.style.display = 'none';
    $emptyState.style.display = 'flex';
    $emptyTitle.textContent = 'Pronto para testar';
    $emptySub.textContent   = 'Selecione uma suite e clique em ▶';
    state.activeTab = null;
  } else {
    activateTab(remaining[remaining.length - 1].id.replace('tab-', ''));
  }
}

// ── Panel ─────────────────────────────────────────────────────────────────────

function createPanel(suiteId, suite, proj) {
  const panel = document.createElement('div');
  panel.className = 'panel';
  panel.id = `panel-${suiteId}`;

  const header = document.createElement('div');
  header.className = 'panel-header';
  header.innerHTML = `
    <div class="panel-title">
      <span class="panel-system">${proj.name}</span>
      <span class="tag ${suite.tag === 'API' ? 'tag-api' : suite.tag === 'Unit' ? 'tag-unit' : 'tag-e2e'}">${suite.tag}</span>
      <span class="panel-name">· ${suite.system} ${suite.name}</span>
    </div>
    <span class="status-badge" id="badge-${suiteId}">Aguardando</span>
    <span class="timer" id="timer-${suiteId}">0.0s</span>
  `;

  const rerunBtn = document.createElement('button');
  rerunBtn.className = 'act-btn';
  rerunBtn.id = `rerun-${suiteId}`;
  rerunBtn.textContent = '↺ Reexecutar';
  rerunBtn.style.display = 'none';
  rerunBtn.addEventListener('click', () => {
    const run = state.runs.get(suiteId);
    if (run) startRun(suiteId, run.projectId);
  });

  const stopBtn = document.createElement('button');
  stopBtn.className = 'act-btn act-danger';
  stopBtn.id = `stop-${suiteId}`;
  stopBtn.textContent = '■ Parar';
  stopBtn.addEventListener('click', () => {
    stopBtn.disabled = true;
    stopBtn.textContent = '... Parando';
    invoke('stop_suite', { suiteId });
  });

  header.appendChild(rerunBtn);
  header.appendChild(stopBtn);

  const term = document.createElement('div');
  term.className = 'terminal';
  term.id = `terminal-${suiteId}`;
  term.innerHTML = '<span class="cursor"></span>';

  const stats = document.createElement('div');
  stats.className = 'stats-bar';
  stats.innerHTML = `
    <span class="stat passed-stat">✓ <b id="pass-${suiteId}">0</b></span>
    <span class="stat failed-stat">✗ <b id="fail-${suiteId}">0</b></span>
    <span class="stat remain-stat">⏳ <b id="remain-${suiteId}">—</b></span>
    <span class="stat total-stat">Total: <b id="total-${suiteId}">—</b></span>
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
  if (run) { run.passCount = 0; run.failCount = 0; run.totalCount = null; }
  updateStats(suiteId);
}

function appendLine(suiteId, line) {
  const term = document.getElementById(`terminal-${suiteId}`);
  if (!term) return;
  term.querySelector('.cursor')?.remove();
  const div = document.createElement('div');
  const cls = classifyLine(line);
  div.className = 'tline ' + cls;
  div.textContent = line;
  term.appendChild(div);
  const cur = document.createElement('span');
  cur.className = 'cursor';
  term.appendChild(cur);
  term.scrollTop = term.scrollHeight;

  // Update real-time stats
  const run = state.runs.get(suiteId);
  if (run) {
    if (cls === 'l-pass') { run.passCount++; updateStats(suiteId); }
    else if (cls === 'l-fail') { run.failCount++; updateStats(suiteId); }
    // Parse total test count from Playwright: "Running 17 tests using 2 workers"
    // or Vitest: "✓ 17 tests"
    const m = line.match(/Running (\d+) test/i) || line.match(/(\d+) tests?\s+(passed|failed|pending)/i);
    if (m && run.totalCount == null) { run.totalCount = parseInt(m[1]); updateStats(suiteId); }
  }
}

function classifyLine(line) {
  const t = line.trim();
  if (/^[✓✔]/.test(t))                               return 'l-pass';
  if (/^[✗×✘]/.test(t))                              return 'l-fail';
  if (/^●/.test(t))                                   return 'l-running';
  if (/^\d+ passed/.test(t) && !/failed/.test(t))    return 'l-sum-pass';
  if (/\d+ failed/.test(t))                          return 'l-sum-fail';
  if (/Error:/i.test(t) && !/^\s{2,}at /.test(line)) return 'l-error';
  if (/^\s{2,}at /.test(line))                       return 'l-stack';
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
  document.getElementById(`stop-${suiteId}`)?.style.setProperty('display', isRunning ? 'flex' : 'none');
  document.getElementById(`rerun-${suiteId}`)?.style.setProperty('display', isRunning ? 'none' : 'flex');
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
    const el = document.getElementById(`timer-${suiteId}`);
    if (el) el.textContent = fmtDuration(duration);
    document.querySelector(`#terminal-${suiteId} .cursor`)?.remove();
  }
  // Reset stop button state when run finishes
  const stopBtn = document.getElementById(`stop-${suiteId}`);
  if (stopBtn) {
    stopBtn.disabled = false;
    stopBtn.textContent = '■ Parar';
  }
  setRunBtn(suiteId, status === 'running');
}

function updateStats(suiteId) {
  const run = state.runs.get(suiteId);
  if (!run) return;
  const set = (id, val) => { const el = document.getElementById(id); if (el) el.textContent = val; };
  const done = run.passCount + run.failCount;
  const remaining = run.totalCount != null ? Math.max(0, run.totalCount - done) : null;
  set(`pass-${suiteId}`,   run.passCount);
  set(`fail-${suiteId}`,   run.failCount);
  set(`remain-${suiteId}`, remaining != null ? remaining : '—');
  set(`total-${suiteId}`,  run.totalCount != null ? run.totalCount : (done > 0 ? done : '—'));
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
