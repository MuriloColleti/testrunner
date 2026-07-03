import { ref, reactive, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// ── State (module-level singletons, shared across all components) ────────────
const projects   = ref([])
const runs       = reactive({})
const lines      = reactive({})
const openTabs   = ref([])
const activeTab  = ref(null)
const currentView = ref('runner')
const collapsed  = reactive({})
const timers     = {}

// ── Utility ──────────────────────────────────────────────────────────────────
function tagClass(tag) {
  return tag === 'API' ? 'tag-api' : tag === 'Unit' ? 'tag-unit' : 'tag-e2e'
}

function fmtMs(ms) {
  if (ms < 1000) return `${ms}ms`
  const s = ms / 1000
  if (s < 60) return `${s.toFixed(1)}s`
  return `${Math.floor(s / 60)}m ${Math.round(s % 60)}s`
}

function runStatus(id) { return runs[id]?.status || '' }
function getRun(id)    { return runs[id] }

function statusLabel(id) {
  return { running: 'Executando', passed: 'Passou', failed: 'Falhou', stopped: 'Parado' }[runStatus(id)] || '\u2014'
}

function fmtElapsed(id) {
  const r = runs[id]
  if (!r) return '\u2014'
  if (r.duration != null) return fmtMs(r.duration)
  if (r.elapsed) return fmtMs(r.elapsed)
  return '\u2014'
}

function remainCount(id) {
  const r = runs[id]
  if (!r || r.totalCount == null) return '\u2014'
  return Math.max(0, r.totalCount - r.passCount - r.failCount)
}

function totalCount(id) {
  const r = runs[id]
  if (!r) return '\u2014'
  if (r.totalCount != null) return r.totalCount
  const d = r.passCount + r.failCount
  return d > 0 ? d : '\u2014'
}

// ── Tag groups ───────────────────────────────────────────────────────────────
const TAG_ORDER = ['E2E', 'API', 'Unit']

function suitesByTag(suites) {
  const map = {}
  for (const s of suites) {
    if (!map[s.tag]) map[s.tag] = []
    map[s.tag].push(s)
  }
  const result = []
  for (const tag of TAG_ORDER) {
    if (map[tag]) result.push({ tag, suites: map[tag] })
  }
  for (const tag of Object.keys(map)) {
    if (!TAG_ORDER.includes(tag)) result.push({ tag, suites: map[tag] })
  }
  return result
}

function isCollapsed(projId, tag) {
  return collapsed[`${projId}-${tag}`] ?? false
}

function toggleGroup(projId, tag) {
  const key = `${projId}-${tag}`
  collapsed[key] = !(collapsed[key] ?? false)
}

// ── Projects ─────────────────────────────────────────────────────────────────
async function loadProjects() {
  try { projects.value = await invoke('get_projects') }
  catch (e) { console.error('[loadProjects] erro:', e) }
}

// ── Timers ───────────────────────────────────────────────────────────────────
function startTimer(suiteId) {
  stopTimer(suiteId)
  timers[suiteId] = setInterval(() => {
    const r = runs[suiteId]
    if (!r) return stopTimer(suiteId)
    r.elapsed = Date.now() - r.startTime
  }, 100)
}

function stopTimer(suiteId) {
  if (timers[suiteId]) { clearInterval(timers[suiteId]); delete timers[suiteId] }
}

// ── Terminal ─────────────────────────────────────────────────────────────────
function classifyLine(line) {
  const t = line.trim()
  // Linhas da pré-validação do backend (sem ✓/✗ para não inflar contadores)
  if (/^\[ERRO\]/.test(t))  return 'l-error'
  if (/^\[AVISO\]/.test(t)) return 'l-running'
  if (/^\[OK\]/.test(t))    return 'l-info'
  // Ruído de build (Vite e afins) usa ✓ sem ser resultado de teste
  if (/modules transformed|built in/.test(t) || /^(dist\/|transforming|rendering chunks|computing gzip)/.test(t))
    return 'l-default'
  // Playwright line-reporter: "ok  N [worker] > ..." (passou)
  if (/^ok\s+\d+\s/.test(t))                                    return 'l-pass'
  // Playwright line-reporter: "x   N [worker] > ..." (falhou)
  if (/^x\s+\d+\s/.test(t))                                     return 'l-fail'
  // Pass unicode: ✓ ✔ √
  if (/[✓✔√]/.test(t) && !/failed/i.test(t))     return 'l-pass'
  if (/^\s*[✓✔√]/.test(line))                     return 'l-pass'
  // Fail unicode: ✕ ✖ ✗ ✘ ×
  if (/[✕✖✗✘×]/.test(t))               return 'l-fail'
  if (/^\s*[✕✖✗✘×]/.test(line))        return 'l-fail'
  if (/^●/.test(t))                                         return 'l-running'
  if (/\d+\s+passed/i.test(t) && !/failed/i.test(t))            return 'l-sum-pass'
  if (/\d+\s+failed/i.test(t))                                   return 'l-sum-fail'
  if (/Error:/i.test(t) && !/^\s{2,}at /.test(line))            return 'l-error'
  if (/^\s{2,}at /.test(line))                                   return 'l-stack'
  if (/Running \d+\s+test/i.test(t))                             return 'l-info'
  return 'l-default'
}

function appendLine(suiteId, text) {
  if (!lines[suiteId]) lines[suiteId] = []
  const cls = classifyLine(text)
  lines[suiteId].push({ text, cls })

  const r = runs[suiteId]
  if (r) {
    if (cls === 'l-pass') r.passCount++
    else if (cls === 'l-fail') r.failCount++

    // Total count from "Running N test(s)"
    const mRun = text.match(/Running\s+(\d+)\s+test/i)
    if (mRun && r.totalCount == null) r.totalCount = parseInt(mRun[1])

    // Summary lines — corrige contadores se a deteccao linha-a-linha falhou
    const mSumPass = text.match(/(\d+)\s+passed/i)
    if (mSumPass) {
      const n = parseInt(mSumPass[1])
      if (n > r.passCount) r.passCount = n
      if (r.totalCount == null) r.totalCount = n + r.failCount
    }
    const mSumFail = text.match(/(\d+)\s+failed/i)
    if (mSumFail) {
      const n = parseInt(mSumFail[1])
      if (n > r.failCount) r.failCount = n
    }

    const mCur = text.match(/\u203a\s+(.+?)(?:\s+\(\d|\s*$)/)
    if (mCur && cls === 'l-default') r.currentTest = mCur[1].trim()
    if (cls === 'l-pass' || cls === 'l-fail') r.currentTest = null
  }

  nextTick(() => {
    const el = document.getElementById(`term-${suiteId}`)
    if (el) el.scrollTop = el.scrollHeight
  })
}

// ── Run / Stop ───────────────────────────────────────────────────────────────
function handleRunBtn(suiteId, projectId) {
  runStatus(suiteId) === 'running' ? stopRun(suiteId) : startRun(suiteId, projectId)
}

// Prepara o estado local de um run (aba, terminal, timer) SEM invocar o
// backend — usado quando a execução já foi iniciada por outro caminho
// (ex.: agendamento disparado pelo scheduler no Rust).
function attachRun(suiteId, projectId) {
  const proj  = projects.value.find(p => p.id === projectId)
  const suite = proj?.suites.find(s => s.id === suiteId)
  if (!proj || !suite) return null

  runs[suiteId] = {
    status: 'running', startTime: Date.now(), elapsed: 0,
    duration: null, passCount: 0, failCount: 0, totalCount: null,
    stopping: false, projectId,
  }
  lines[suiteId] = []

  ensureTab(suiteId, suite, proj)
  activateTab(suiteId)
  startTimer(suiteId)

  return { proj, suite }
}

function startRun(suiteId, projectId) {
  const attached = attachRun(suiteId, projectId)
  if (!attached) return
  const { proj, suite } = attached

  currentView.value = 'runner'

  invoke('run_suite', {
    projectId:    proj.id,    projectName:  proj.name,
    projectPath:  proj.path,  suiteId,
    suiteName:    suite.name, suiteTag:     suite.tag,
    suiteCommand: suite.command, suiteCwd:  suite.cwd,
    suiteArgs:    suite.args,
  }).catch(e => appendLine(suiteId, `Erro: ${e}`))
}

function stopRun(suiteId) {
  const r = runs[suiteId]
  if (r) r.stopping = true
  invoke('stop_suite', { suiteId })
}

// ── Tabs ─────────────────────────────────────────────────────────────────────
function ensureTab(suiteId, suite, proj) {
  if (!openTabs.value.find(t => t.suiteId === suiteId)) {
    openTabs.value.push({
      suiteId,
      suiteName: suite.name,
      suiteTag:  suite.tag,
      projName:  proj.name,
      projectId: proj.id,
    })
  }
}

function activateTab(suiteId) {
  if (openTabs.value.find(t => t.suiteId === suiteId)) activeTab.value = suiteId
}

function closeTab(suiteId) {
  if (runStatus(suiteId) === 'running') invoke('stop_suite', { suiteId })
  stopTimer(suiteId)
  delete runs[suiteId]
  delete lines[suiteId]
  openTabs.value = openTabs.value.filter(t => t.suiteId !== suiteId)
  if (activeTab.value === suiteId)
    activeTab.value = openTabs.value.at(-1)?.suiteId ?? null
}

// ── Event listeners ──────────────────────────────────────────────────────────
let listenersRegistered = false

async function registerListeners() {
  if (listenersRegistered) return
  listenersRegistered = true

  await listen('suite-started', ({ payload }) => {
    const r = runs[payload.suiteId]
    if (r) { r.status = 'running'; r.startTime = Date.now() }
  })
  await listen('suite-output', ({ payload }) => {
    appendLine(payload.suiteId, payload.line)
  })
  await listen('suite-done', ({ payload }) => {
    const r = runs[payload.suiteId]
    if (r) { r.status = payload.status; r.duration = payload.duration; r.stopping = false }
    stopTimer(payload.suiteId)
  })
}

// ── Export ────────────────────────────────────────────────────────────────────
export function useTestState() {
  return {
    projects, runs, lines, openTabs, activeTab, currentView, collapsed,
    tagClass, fmtMs, runStatus, getRun, statusLabel, fmtElapsed,
    remainCount, totalCount, classifyLine,
    loadProjects, registerListeners,
    suitesByTag, isCollapsed, toggleGroup,
    attachRun, startRun, stopRun, handleRunBtn,
    ensureTab, activateTab, closeTab,
  }
}
