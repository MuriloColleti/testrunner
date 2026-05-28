<template>
  <div class="layout">

    <!-- ── Sidebar ─────────────────────────────────────────── -->
    <aside class="sidebar">
      <div class="sidebar-header">
        <div class="logo">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="18" height="18">
            <path d="M12 2L3 7v10l9 5 9-5V7L12 2z" stroke-linejoin="round"/>
            <path d="M12 2v20M3 7l9 5 9-5" stroke-linejoin="round"/>
          </svg>
          <span>Report<b>Test</b></span>
        </div>
        <button class="add-btn" @click="openModal()" title="Novo projeto">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
            <path d="M8 2v12M2 8h12"/>
          </svg>
        </button>
      </div>

      <div class="sidebar-body">
        <div v-if="projects.length === 0" class="sidebar-msg">
          Nenhum projeto.<br>Clique em <b>+</b> para adicionar.
        </div>
        <template v-else>
          <div v-for="proj in projects" :key="proj.id" class="project-block">
            <div class="project-hd">
              <span class="project-name">{{ proj.name }}</span>
              <button class="icon-btn" @click="openModal(proj.id)" title="Editar">
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="11" height="11">
                  <path d="M11.5 2.5l2 2L5 13H3v-2L11.5 2.5z"/>
                </svg>
              </button>
            </div>
            <div
              v-for="s in proj.suites"
              :key="s.id"
              class="suite-row"
              :class="{ active: activeTab === s.id }"
              @click="activateTab(s.id)"
            >
              <span class="suite-dot" :class="runStatus(s.id)"></span>
              <div class="suite-info">
                <span class="suite-label">{{ s.system }} · {{ s.name }}</span>
                <span class="tag" :class="tagClass(s.tag)">{{ s.tag }}</span>
              </div>
              <button
                class="run-btn"
                :class="{ running: runStatus(s.id) === 'running' }"
                @click.stop="handleRunBtn(s.id, proj.id)"
                :title="runStatus(s.id) === 'running' ? 'Parar' : 'Executar'"
              >
                <svg v-if="runStatus(s.id) !== 'running'" viewBox="0 0 10 12" fill="currentColor" width="9" height="10">
                  <path d="M1 1l8 5-8 5V1z"/>
                </svg>
                <svg v-else viewBox="0 0 10 10" fill="currentColor" width="9" height="9">
                  <rect x="1" y="1" width="3" height="8"/><rect x="6" y="1" width="3" height="8"/>
                </svg>
              </button>
            </div>
          </div>
        </template>
      </div>
    </aside>

    <!-- ── Main ───────────────────────────────────────────── -->
    <main class="main">

      <!-- Tabs + History button -->
      <div class="tabs-bar">
        <div
          v-for="tab in openTabs"
          :key="tab.suiteId"
          class="tab"
          :class="{ active: activeTab === tab.suiteId }"
          @click="activateTab(tab.suiteId)"
        >
          <span class="tab-dot" :class="runStatus(tab.suiteId)"></span>
          <span class="tab-label">{{ tab.projName }} <span class="tab-sep">›</span> {{ tab.suiteName }}</span>
          <span class="tab-close" @click.stop="closeTab(tab.suiteId)">×</span>
        </div>
        <div class="tabs-spacer"></div>
        <button
          class="history-toggle"
          :class="{ active: hist.open }"
          @click="toggleHistory"
          title="Histórico de execuções"
        >
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
            <circle cx="8" cy="8" r="6"/>
            <path d="M8 5v3.5l2.5 1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span>Histórico</span>
        </button>
      </div>

      <!-- History panel -->
      <Transition name="hist">
        <div v-if="hist.open" class="hist-panel">
          <!-- Filters -->
          <div class="hist-filters">
            <span class="hist-filters-lbl">Últimas</span>
            <button
              v-for="w in [1,2,3,4]" :key="w"
              class="hf-btn"
              :class="{ active: hist.weeks === w }"
              @click="hist.weeks = w"
            >{{ w }} sem</button>
            <span class="hist-count">{{ filteredRuns.length }} execuç{{ filteredRuns.length === 1 ? 'ão' : 'ões' }}</span>
          </div>

          <!-- Run list -->
          <div class="hist-list">
            <div v-if="hist.loading" class="hist-empty">Carregando...</div>
            <div v-else-if="filteredRuns.length === 0" class="hist-empty">
              Sem execuções nas últimas {{ hist.weeks }} semana{{ hist.weeks > 1 ? 's' : '' }}.
            </div>
            <div
              v-for="run in filteredRuns"
              :key="run.id"
              class="hist-item"
              @click="openHistoryRun(run)"
            >
              <span class="hi-dot" :class="run.status"></span>
              <div class="hi-info">
                <span class="hi-name">{{ run.projectName }} <span class="hi-sep">›</span> {{ run.suiteName }}</span>
                <span class="hi-date">{{ formatRunDate(run.startedAt) }}</span>
              </div>
              <div class="hi-right">
                <span class="hi-stats">
                  <span class="hi-pass">{{ run.passCount }}</span>
                  <span class="hi-slash">/</span>
                  <span class="hi-fail">{{ run.failCount }}</span>
                </span>
                <span class="hi-dur">{{ fmtMs(run.durationMs) }}</span>
                <span class="tag sm" :class="tagClass(run.suiteTag)">{{ run.suiteTag }}</span>
              </div>
            </div>
          </div>
        </div>
      </Transition>
      <div v-if="hist.open" class="hist-overlay" @click="hist.open = false"></div>

      <!-- Content area -->
      <div class="content">

        <!-- Empty state -->
        <div v-if="openTabs.length === 0" class="empty-state">
          <div class="empty-icon">
            <svg viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="1.5" width="48" height="48">
              <polygon points="10,6 38,24 10,42" stroke-linejoin="round"/>
            </svg>
          </div>
          <p class="empty-title">{{ projects.length === 0 ? 'Nenhum projeto configurado' : 'Pronto para executar' }}</p>
          <p class="empty-sub">{{ projects.length === 0 ? 'Adicione um projeto para começar' : 'Clique em ▶ em um teste na barra lateral' }}</p>
          <button v-if="projects.length === 0" class="btn-primary" @click="openModal()">+ Adicionar Projeto</button>
        </div>

        <!-- Test panels -->
        <div
          v-for="tab in openTabs"
          :key="tab.suiteId"
          class="panel"
          :class="{ active: activeTab === tab.suiteId }"
        >
          <!-- Panel header -->
          <div class="panel-header">
            <div class="panel-title">
              <span class="breadcrumb">
                <span class="bc-proj">{{ tab.projName }}</span>
                <span class="bc-sep">›</span>
                <span class="bc-suite">{{ tab.suiteName }}</span>
              </span>
              <span class="tag sm" :class="tagClass(tab.suiteTag)">{{ tab.suiteTag }}</span>
            </div>
            <div class="panel-status">
              <span class="status-badge" :class="runStatus(tab.suiteId)">
                <span class="status-dot"></span>
                {{ statusLabel(tab.suiteId) }}
              </span>
              <span class="timer">{{ fmtElapsed(tab.suiteId) }}</span>
              <button
                v-if="(lines[tab.suiteId] || []).length > 0"
                class="export-btn"
                @click.stop="exportReport(tab.suiteId, tab)"
                title="Exportar relatório HTML"
              >
                <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
                  <path d="M7 1v8M4 6l3 3 3-3" stroke-linecap="round" stroke-linejoin="round"/>
                  <path d="M2 10v2a1 1 0 001 1h8a1 1 0 001-1v-2" stroke-linecap="round"/>
                </svg>
              </button>
            </div>
            <div class="panel-actions">
              <button
                v-if="runStatus(tab.suiteId) === 'running'"
                class="act-btn danger"
                :disabled="getRun(tab.suiteId)?.stopping"
                @click="stopRun(tab.suiteId)"
              >
                <svg viewBox="0 0 10 10" fill="currentColor" width="9" height="9">
                  <rect x="1" y="1" width="8" height="8"/>
                </svg>
                {{ getRun(tab.suiteId)?.stopping ? 'Parando...' : 'Parar' }}
              </button>
              <button v-else class="act-btn" @click="startRun(tab.suiteId, tab.projectId)">
                <svg viewBox="0 0 10 12" fill="currentColor" width="9" height="10"><path d="M1 1l8 5-8 5V1z"/></svg>
                Reexecutar
              </button>
            </div>
          </div>

          <!-- Stats bar -->
          <div class="stats-bar">
            <div class="stat pass">
              <span class="stat-num">{{ getRun(tab.suiteId)?.passCount ?? 0 }}</span>
              <span class="stat-lbl">Passou</span>
            </div>
            <div class="stat fail">
              <span class="stat-num">{{ getRun(tab.suiteId)?.failCount ?? 0 }}</span>
              <span class="stat-lbl">Falhou</span>
            </div>
            <div class="stat pending">
              <span class="stat-num">{{ remainCount(tab.suiteId) }}</span>
              <span class="stat-lbl">Restantes</span>
            </div>
            <div class="stat-divider"></div>
            <div class="stat total">
              <span class="stat-num">{{ totalCount(tab.suiteId) }}</span>
              <span class="stat-lbl">Total</span>
            </div>
          </div>

          <!-- Terminal -->
          <div class="terminal" :id="`term-${tab.suiteId}`">
            <div
              v-for="(line, i) in lines[tab.suiteId] || []"
              :key="i"
              class="tline"
              :class="line.cls"
            >{{ line.text }}</div>
            <span v-if="runStatus(tab.suiteId) === 'running'" class="cursor"></span>
          </div>
        </div>

      </div>
    </main>

    <!-- ── Config Modal ──────────────────────────────────── -->
    <Transition name="modal">
      <div v-if="modal.open" class="modal-overlay" @click.self="closeModal">
        <div class="modal">
          <div class="modal-header">
            <span>{{ modal.title }}</span>
            <button class="modal-close" @click="closeModal">×</button>
          </div>

          <div class="modal-body">
            <div class="form-group">
              <label>Nome do Projeto</label>
              <input
                v-model="modal.name"
                type="text"
                placeholder="Ex: Fluke, Venturus..."
                autocomplete="off"
                @keydown.enter="focusPath"
              />
            </div>

            <div class="form-group">
              <label>Caminho do Projeto</label>
              <div class="path-row">
                <input
                  ref="pathInput"
                  v-model="modal.path"
                  type="text"
                  placeholder="C:\caminho\para\o\projeto"
                  autocomplete="off"
                  @keydown.enter="verifyPath"
                />
                <button class="browse-btn" @click="browsePath" :disabled="modal.verifying">
                  <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
                    <path d="M1 4.5A1.5 1.5 0 012.5 3H6l1.5 2H13.5A1.5 1.5 0 0115 6.5v6A1.5 1.5 0 0113.5 14h-11A1.5 1.5 0 011 12.5v-8z"/>
                  </svg>
                  Procurar
                </button>
              </div>
              <span class="form-hint">Pasta raiz com <code>playwright.config.ts</code> ou <code>vitest.config.ts</code></span>
            </div>

            <button class="btn-verify" @click="verifyPath" :disabled="modal.verifying">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
                <circle cx="6.5" cy="6.5" r="4"/><path d="M11 11l3 3"/>
              </svg>
              {{ modal.verifying ? 'Verificando...' : 'Verificar Caminho' }}
            </button>

            <Transition name="fade">
              <div v-if="modal.verifyMsg" class="verify-result" :class="modal.verifyOk ? 'ok' : 'error'">
                {{ modal.verifyMsg }}
              </div>
            </Transition>
          </div>

          <div class="modal-footer">
            <button v-if="modal.editingId" class="btn-danger" @click="deleteProject">Remover</button>
            <div class="footer-right">
              <button class="btn-secondary" @click="closeModal">Cancelar</button>
              <button class="btn-primary" @click="saveProject" :disabled="!modal.pendingSuites.length">Salvar</button>
            </div>
          </div>
        </div>
      </div>
    </Transition>

  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, nextTick, toRaw } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// ── State ─────────────────────────────────────────────────────────────────────
const projects  = ref([])
const activeTab = ref(null)
const openTabs  = ref([])   // [{ suiteId, suiteName, suiteTag, projName, projectId }]
const runs      = reactive({}) // [suiteId]: RunState
const lines     = reactive({}) // [suiteId]: [{ text, cls }]
const timers    = {}           // [suiteId]: intervalId

// ── History state ─────────────────────────────────────────────────────────────
const hist = reactive({ open: false, weeks: 1, runs: [], loading: false })

const filteredRuns = computed(() => {
  const cutoff = Date.now() - hist.weeks * 7 * 24 * 60 * 60 * 1000
  return hist.runs.filter(r => {
    const ts = new Date(r.startedAt.replace(' ', 'T')).getTime()
    return !isNaN(ts) && ts >= cutoff
  })
})

// ── Modal state ───────────────────────────────────────────────────────────────
const modal = reactive({
  open: false, title: '', editingId: null,
  name: '', path: '',
  pendingSuites: [], verifying: false,
  verifyOk: false, verifyMsg: '',
})
const pathInput = ref(null)

// ── Boot ──────────────────────────────────────────────────────────────────────
onMounted(async () => {
  await loadProjects()
  await registerListeners()
})

async function loadProjects() {
  try { projects.value = await invoke('get_projects') }
  catch (e) { console.error('[loadProjects] erro:', e) }
}

async function registerListeners() {
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

// ── Helpers ───────────────────────────────────────────────────────────────────
function runStatus(id) { return runs[id]?.status || '' }
function getRun(id)    { return runs[id] }

function statusLabel(id) {
  return { running: 'Executando', passed: 'Passou', failed: 'Falhou', stopped: 'Parado' }[runStatus(id)] || '—'
}

function tagClass(tag) {
  return tag === 'API' ? 'tag-api' : tag === 'Unit' ? 'tag-unit' : 'tag-e2e'
}

function remainCount(id) {
  const r = runs[id]
  if (!r || r.totalCount == null) return '—'
  return Math.max(0, r.totalCount - r.passCount - r.failCount)
}
function totalCount(id) {
  const r = runs[id]
  if (!r) return '—'
  if (r.totalCount != null) return r.totalCount
  const d = r.passCount + r.failCount
  return d > 0 ? d : '—'
}

function fmtElapsed(id) {
  const r = runs[id]
  if (!r) return '—'
  if (r.duration != null) return fmtMs(r.duration)
  if (r.elapsed) return fmtMs(r.elapsed)
  return '—'
}
function fmtMs(ms) {
  if (ms < 1000) return `${ms}ms`
  const s = ms / 1000
  if (s < 60) return `${s.toFixed(1)}s`
  return `${Math.floor(s / 60)}m ${Math.round(s % 60)}s`
}

// ── Export PDF ────────────────────────────────────────────────────────────────

// jsPDF built-in fonts cover only Latin-1 (Windows-1252).
function pdfSafe(s) {
  return String(s)
    .replace(/[✓✔]/g, '+ ')
    .replace(/[✗✘]/g, '\xD7 ')
    .replace(/●/g,    '* ')
    .replace(/[^\x00-\xFF]/g, '?')
}

// Extrai apenas as linhas relevantes: falhas, erros, stacks e resumo
function extractFailedLines(out) {
  const result = []
  let inFail = false
  for (const line of out) {
    if (line.cls === 'l-fail') {
      inFail = true
      result.push(line)
    } else if (line.cls === 'l-error') {
      inFail = true
      result.push(line)
    } else if (line.cls === 'l-stack' && inFail) {
      result.push(line)
    } else if (line.cls === 'l-default' && inFail) {
      // Linhas de contexto (Expected / Received) logo após uma falha
      result.push(line)
    } else if (line.cls === 'l-sum-fail' || line.cls === 'l-sum-pass') {
      result.push(line)
      inFail = false
    } else if (line.cls === 'l-pass' || line.cls === 'l-info') {
      inFail = false
    }
  }
  return result
}

async function exportReport(suiteId, tab) {
  console.log('[export] clicado', { suiteId, tab })
  try {
    const { jsPDF } = await import('jspdf')

    const run  = runs[suiteId]
    const out  = lines[suiteId] || []
    const now  = new Date()
    const date = now.toLocaleString('pt-BR')
    const st   = runStatus(suiteId)
    const lbl  = { running:'Executando', passed:'Passou', failed:'Falhou', stopped:'Parado' }[st] || '-'
    const dur  = fmtElapsed(suiteId)
    const pass = run?.passCount ?? 0
    const fail = run?.failCount ?? 0
    const tot  = run?.totalCount ?? (pass + fail || '-')
    const failedLines = extractFailedLines(out)

    console.log('[export] dados:', { st, pass, fail, tot, dur, falhas: failedLines.length })

    // ── Palette ─────────────────────────────────────────────────────────────
    const BG   = [13,  17,  23]
    const CARD = [22,  27,  34]
    const BORD = [33,  38,  45]
    const TEXT = [230, 237, 243]
    const MUTE = [139, 148, 158]
    const DIM  = [72,  79,  88]
    const GRN  = [63,  185, 80]
    const RED  = [248, 81,  73]
    const BLU  = [88,  166, 255]
    const AMB  = [210, 153, 34]
    const PUR  = [188, 140, 255]

    const stColor  = { passed:GRN, failed:RED, running:AMB, stopped:MUTE }[st] || MUTE
    const tagColor = tab.suiteTag === 'API' ? PUR : tab.suiteTag === 'Unit' ? GRN : BLU

    // ── Doc setup ───────────────────────────────────────────────────────────
    console.log('[export] criando documento jsPDF...')
    const doc = new jsPDF({ unit: 'pt', format: 'a4' })
    console.log('[export] doc criado:', typeof doc)
    const W   = doc.internal.pageSize.getWidth()
    const H   = doc.internal.pageSize.getHeight()
    const ML  = 40, MR = 40
    const CW  = W - ML - MR
    const BOT = H - 30

    const setF = c => doc.setFillColor(c[0], c[1], c[2])
    const setT = c => doc.setTextColor(c[0], c[1], c[2])
    const setD = c => doc.setDrawColor(c[0], c[1], c[2])
    const mix  = (c, a) => c.map((v, i) => Math.round(v * a + BG[i] * (1 - a)))
    const hr   = (yy, color = BORD) => { setD(color); doc.setLineWidth(0.5); doc.line(ML, yy, W - MR, yy) }

    const pageBg = () => { setF(BG); doc.rect(0, 0, W, H, 'F') }
    pageBg()

    let y = 44

    // ── Header ──────────────────────────────────────────────────────────────
    // Brand + date on same line
    doc.setFont('helvetica', 'bold')
    doc.setFontSize(7)
    setT(BLU)
    doc.text('REPORTTEST', ML, y)
    doc.setFont('helvetica', 'normal')
    setT(DIM)
    doc.text(date, W - MR, y, { align: 'right' })
    y += 10
    hr(y)
    y += 16

    // Project › Suite
    doc.setFont('helvetica', 'bold')
    doc.setFontSize(17)
    setT(TEXT)
    const pTxt = pdfSafe(tab.projName)
    const sTxt = pdfSafe(tab.suiteName)
    const sep  = '  \xBB  '
    doc.text(pTxt, ML, y)
    setT(DIM)
    doc.text(sep, ML + doc.getTextWidth(pTxt), y)
    setT(TEXT)
    doc.text(sTxt, ML + doc.getTextWidth(pTxt) + doc.getTextWidth(sep), y)

    // Tag pill top-right
    doc.setFont('helvetica', 'bold')
    doc.setFontSize(7.5)
    const tagTxt = tab.suiteTag.toUpperCase()
    const tagPW  = doc.getTextWidth(tagTxt) + 14
    const tagX   = W - MR - tagPW
    setF(mix(tagColor, 0.2))
    doc.roundedRect(tagX, y - 12, tagPW, 15, 4, 4, 'F')
    setT(tagColor)
    doc.text(tagTxt, tagX + 7, y - 1)
    y += 14

    // Status indicator
    doc.setFont('helvetica', 'bold')
    doc.setFontSize(9)
    setT(stColor)
    doc.text(lbl, ML, y)
    y += 22

    // ── Stats (4 cards) ──────────────────────────────────────────────────────
    const bw = (CW - 3) / 4
    const bh = 58
    ;[
      { val: String(pass), lbl: 'PASSOU',         clr: GRN                    },
      { val: String(fail), lbl: 'FALHOU',         clr: fail > 0 ? RED : MUTE  },
      { val: String(tot),  lbl: 'TOTAL',          clr: MUTE                   },
      { val: dur,          lbl: 'DURA\xC7\xC3O',  clr: MUTE                   },
    ].forEach((s, i) => {
      const bx = ML + i * (bw + 1)
      setF(CARD)
      doc.rect(bx, y, bw, bh, 'F')
      // colored accent bar on top
      setF(s.clr)
      doc.rect(bx, y, bw, 3, 'F')
      doc.setFont('courier', 'bold')
      doc.setFontSize(i < 3 ? 22 : 15)
      setT(s.clr)
      doc.text(s.val, bx + 12, y + 36)
      doc.setFont('helvetica', 'bold')
      doc.setFontSize(7)
      setT(DIM)
      doc.text(s.lbl, bx + 12, y + 51)
    })
    y += bh + 22

    // ── Failures section ─────────────────────────────────────────────────────
    if (fail > 0 && failedLines.length > 0) {
      // Section heading
      doc.setFont('helvetica', 'bold')
      doc.setFontSize(8)
      setT(RED)
      const heading = `TESTES COM FALHA  (${fail})`
      doc.text(heading, ML, y)
      hr(y + 4, mix(RED, 0.4))
      y += 18

      const LH   = 11
      const TERM = [1, 4, 9]
      const newPage = () => {
        doc.addPage(); pageBg()
        setF(TERM); doc.rect(ML, 20, CW, BOT - 20, 'F')
        y = 30
      }

      // Terminal bg for rest of page
      setF(TERM)
      doc.rect(ML, y, CW, BOT - y, 'F')
      y += 8

      doc.setFontSize(8)
      failedLines.forEach(line => {
        const isFailTitle = line.cls === 'l-fail'
        const isError     = line.cls === 'l-error'
        const isStack     = line.cls === 'l-stack'
        const isSumFail   = line.cls === 'l-sum-fail'
        const isSumPass   = line.cls === 'l-sum-pass'
        const isContext   = line.cls === 'l-default'

        // Gap before each new failed test title
        if (isFailTitle && y > 200) y += 6

        // Summary lines get extra spacing
        if (isSumFail || isSumPass) y += 6

        const color  = isFailTitle ? RED : isError ? RED : isStack ? DIM
                     : isSumFail   ? RED : isSumPass ? GRN : MUTE
        const indent = isStack ? 20 : isError ? 10 : isContext ? 10 : 0

        const txt     = pdfSafe(line.text)
        const maxW    = CW - indent - 24   // 24pt de margem interna para não vazar
        const wrapped = doc.splitTextToSize(txt, maxW)

        wrapped.forEach((sl, wi) => {
          if (y + LH > BOT) newPage()
          // Error message background
          if (isError && wi === 0) {
            setF(mix(RED, 0.1))
            doc.rect(ML + 2, y - LH + 2, CW - 4, LH * wrapped.length + 2, 'F')
          }
          doc.setFont('courier', isFailTitle || isSumFail ? 'bold' : 'normal')
          setT(color)
          doc.text(sl, ML + indent, y)
          y += LH
        })
      })

    } else if (fail === 0) {
      // All passed banner
      setF(mix(GRN, 0.08))
      doc.rect(ML, y, CW, 40, 'F')
      setD(mix(GRN, 0.35))
      doc.setLineWidth(0.5)
      doc.rect(ML, y, CW, 40, 'D')
      doc.setFont('helvetica', 'bold')
      doc.setFontSize(11)
      setT(GRN)
      doc.text('Todos os testes passaram!', ML + 16, y + 24)
    }

    // ── Footer on every page ─────────────────────────────────────────────────
    const pages = doc.internal.getNumberOfPages()
    for (let p = 1; p <= pages; p++) {
      doc.setPage(p)
      hr(BOT + 6)
      doc.setFont('helvetica', 'normal')
      doc.setFontSize(7)
      setT(DIM)
      doc.text('ReportTest', ML, BOT + 18)
      doc.text(
        `${pdfSafe(tab.projName)} \xB7 ${pdfSafe(tab.suiteName)} \xB7 ${date}   ${p}/${pages}`,
        W - MR, BOT + 18, { align: 'right' }
      )
    }

    // ── Save via Rust ────────────────────────────────────────────────────────
    console.log('[export] gerando bytes...')
    const safe     = `${tab.projName}-${tab.suiteName}`.replace(/[^a-z0-9]/gi, '-').toLowerCase()
    const df       = now.toISOString().slice(0, 10)
    const filename = `report-${safe}-${df}.pdf`
    const buffer   = doc.output('arraybuffer')
    const data     = Array.from(new Uint8Array(buffer))
    console.log('[export] bytes prontos:', data.length, '| filename:', filename)

    const savedPath = await invoke('save_pdf', { filename, data })
    console.log('[export] salvo em:', savedPath)

  } catch (err) {
    console.error('[exportReport] ERRO:', err)
    alert(`Erro ao gerar PDF:\n${err?.message ?? err}`)
  }
}

// ── Terminal ──────────────────────────────────────────────────────────────────
function appendLine(suiteId, text) {
  if (!lines[suiteId]) lines[suiteId] = []
  const cls = classifyLine(text)
  lines[suiteId].push({ text, cls })

  const r = runs[suiteId]
  if (r) {
    if (cls === 'l-pass') r.passCount++
    else if (cls === 'l-fail') r.failCount++
    const m = text.match(/Running (\d+) test/i)
    if (m && r.totalCount == null) r.totalCount = parseInt(m[1])
  }

  nextTick(() => {
    const el = document.getElementById(`term-${suiteId}`)
    if (el) el.scrollTop = el.scrollHeight
  })
}

function classifyLine(line) {
  const t = line.trim()
  if (/^[✓✔]/.test(t))                               return 'l-pass'
  if (/^[✗×✘]/.test(t))                              return 'l-fail'
  if (/^●/.test(t))                                   return 'l-running'
  if (/^\d+ passed/.test(t) && !/failed/.test(t))    return 'l-sum-pass'
  if (/\d+ failed/.test(t))                          return 'l-sum-fail'
  if (/Error:/i.test(t) && !/^\s{2,}at /.test(line)) return 'l-error'
  if (/^\s{2,}at /.test(line))                       return 'l-stack'
  if (/^Running \d+/.test(t))                         return 'l-info'
  return 'l-default'
}

// ── Run / Stop ─────────────────────────────────────────────────────────────────
function handleRunBtn(suiteId, projectId) {
  runStatus(suiteId) === 'running' ? stopRun(suiteId) : startRun(suiteId, projectId)
}

function startRun(suiteId, projectId) {
  const proj  = projects.value.find(p => p.id === projectId)
  const suite = proj?.suites.find(s => s.id === suiteId)
  if (!proj || !suite) return

  runs[suiteId] = {
    status: 'running', startTime: Date.now(), elapsed: 0,
    duration: null, passCount: 0, failCount: 0, totalCount: null,
    stopping: false, projectId,
  }
  lines[suiteId] = []

  ensureTab(suiteId, suite, proj)
  activateTab(suiteId)
  startTimer(suiteId)

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

// ── Tabs ──────────────────────────────────────────────────────────────────────
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

// ── History ───────────────────────────────────────────────────────────────────
async function toggleHistory() {
  if (hist.open) { hist.open = false; return }
  hist.open = true
  hist.loading = true
  try {
    hist.runs = await invoke('get_runs', { projectId: null })
  } catch (e) {
    console.error('[history]', e)
  } finally {
    hist.loading = false
  }
}

async function openHistoryRun(run) {
  const proj  = projects.value.find(p => p.id === run.projectId)
  const suite = proj?.suites.find(s => s.id === run.suiteId)
  if (!proj || !suite) return

  ensureTab(run.suiteId, suite, proj)
  activateTab(run.suiteId)
  hist.open = false

  // Don't overwrite a currently running suite
  if (runStatus(run.suiteId) === 'running') return

  // Restore historical run state (stats + status)
  runs[run.suiteId] = {
    status:     run.status,
    startTime:  null,
    elapsed:    0,
    duration:   run.durationMs,
    passCount:  run.passCount,
    failCount:  run.failCount,
    totalCount: run.passCount + run.failCount,
    stopping:   false,
    projectId:  run.projectId,
  }

  // Load the saved output lines
  try {
    const outputLines = await invoke('get_run_output', { runId: run.id })
    lines[run.suiteId] = outputLines.map(text => ({ text, cls: classifyLine(text) }))
  } catch (e) {
    console.error('[history] erro ao carregar output:', e)
    lines[run.suiteId] = [{ text: '(output não disponível para esta execução)', cls: 'l-default' }]
  }

  await nextTick()
  const el = document.getElementById(`term-${run.suiteId}`)
  if (el) el.scrollTop = el.scrollHeight
}

function formatRunDate(str) {
  const d = new Date(str.replace(' ', 'T'))
  if (isNaN(d)) return str
  const now   = new Date()
  const diffMs = now - d
  const diffD  = Math.floor(diffMs / 86400000)
  const time   = d.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' })
  if (diffD === 0) return `Hoje, ${time}`
  if (diffD === 1) return `Ontem, ${time}`
  if (diffD < 7)   return `${d.toLocaleDateString('pt-BR', { weekday: 'short' })}, ${time}`
  return `${d.toLocaleDateString('pt-BR', { day: '2-digit', month: '2-digit' })}, ${time}`
}

// ── Timer ─────────────────────────────────────────────────────────────────────
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

// ── Modal ─────────────────────────────────────────────────────────────────────
function openModal(projectId = null) {
  Object.assign(modal, {
    open: true, editingId: projectId,
    title: projectId ? 'Editar Projeto' : 'Novo Projeto',
    name: '', path: '', pendingSuites: [],
    verifying: false, verifyOk: false, verifyMsg: '',
  })
  if (projectId) {
    const p = projects.value.find(x => x.id === projectId)
    if (p) {
      modal.name = p.name; modal.path = p.path
      modal.pendingSuites = p.suites
      modal.verifyOk = true
      modal.verifyMsg = `✓ ${p.suites.length} suite(s) encontrada(s).`
    }
  }
  nextTick(() => document.querySelector('.modal input')?.focus())
}

function closeModal() { modal.open = false }

function focusPath() { pathInput.value?.focus() }

async function browsePath() {
  modal.verifying = true
  try {
    const folder = await invoke('pick_folder')
    if (folder) {
      modal.path = folder
      if (!modal.name.trim()) {
        const parts = folder.replace(/\\/g, '/').split('/')
        modal.name = parts.at(-1) || ''
      }
      await verifyPath()
    }
  } finally { modal.verifying = false }
}

async function verifyPath() {
  if (!modal.path.trim()) {
    modal.verifyOk = false; modal.verifyMsg = 'Informe o caminho.'; return
  }
  modal.verifying = true; modal.verifyMsg = ''
  try {
    const suites = await invoke('scan_project', { path: modal.path.trim() })
    modal.pendingSuites = suites
    modal.verifyOk  = true
    modal.verifyMsg = `✓ ${suites.length} suite(s) encontrada(s).`
  } catch (e) {
    modal.pendingSuites = []
    modal.verifyOk  = false
    modal.verifyMsg = String(e)
  } finally { modal.verifying = false }
}

async function saveProject() {
  if (!modal.name.trim() || !modal.path.trim()) return
  if (!modal.pendingSuites.length) { await verifyPath(); return }
  const byPath = projects.value.find(p => p.path === modal.path.trim())
  const project = {
    id:     modal.editingId ?? byPath?.id ?? crypto.randomUUID(),
    name:   modal.name.trim(),
    path:   modal.path.trim(),
    suites: toRaw(modal.pendingSuites).map(s => toRaw(s)),
  }
  try {
    await invoke('save_project', { project })
    await loadProjects()
    closeModal()
  } catch (e) {
    console.error('[saveProject] erro:', e)
    modal.verifyOk = false; modal.verifyMsg = `Erro ao salvar: ${e}`
  }
}

async function deleteProject() {
  if (!confirm('Remover este projeto?')) return
  try {
    await invoke('delete_project', { projectId: modal.editingId })
    await loadProjects(); closeModal()
  } catch (e) { console.error(e) }
}
</script>
