<template>
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
      <span class="tab-label">{{ tab.projName }} <span class="tab-sep">&#x203a;</span> {{ tab.suiteName }}</span>
      <span class="tab-close" @click.stop="closeTab(tab.suiteId)">&times;</span>
    </div>
    <div class="tabs-spacer"></div>
    <button
      class="history-toggle"
      :class="{ active: hist.open }"
      @click="toggleHistory"
      title="Historico de execucoes"
    >
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
        <circle cx="8" cy="8" r="6"/>
        <path d="M8 5v3.5l2.5 1.5" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      <span>Historico</span>
    </button>
  </div>

  <!-- History panel -->
  <Transition name="hist">
    <div v-if="hist.open" class="hist-panel">
      <div class="hist-filters">
        <span class="hist-filters-lbl">Ultimas</span>
        <button
          v-for="w in [1,2,3,4]" :key="w"
          class="hf-btn"
          :class="{ active: hist.weeks === w }"
          @click="hist.weeks = w"
        >{{ w }} sem</button>
        <span class="hist-count">{{ filteredRuns.length }} execuc{{ filteredRuns.length === 1 ? 'ao' : 'oes' }}</span>
      </div>
      <div class="hist-list">
        <div v-if="hist.loading" class="hist-empty">Carregando...</div>
        <div v-else-if="filteredRuns.length === 0" class="hist-empty">
          Sem execucoes nas ultimas {{ hist.weeks }} semana{{ hist.weeks > 1 ? 's' : '' }}.
        </div>
        <div
          v-for="run in filteredRuns"
          :key="run.id"
          class="hist-item"
          @click="openHistoryRun(run)"
        >
          <span class="hi-dot" :class="run.status"></span>
          <div class="hi-info">
            <span class="hi-name">{{ run.projectName }} <span class="hi-sep">&#x203a;</span> {{ run.suiteName }}</span>
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
      <p class="empty-sub">{{ projects.length === 0 ? 'Adicione um projeto para comecar' : 'Clique em \u25b6 em um teste na barra lateral' }}</p>
      <button v-if="projects.length === 0" class="btn-primary" @click="emit('add-project')">+ Adicionar Projeto</button>
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
            <span class="bc-sep">&#x203a;</span>
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
            title="Exportar relatorio PDF"
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
        <div v-if="getRun(tab.suiteId)?.currentTest" class="stat-current">
          <span class="stat-cur-dot"></span>
          <span class="stat-cur-txt">{{ getRun(tab.suiteId).currentTest }}</span>
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
</template>

<script setup>
import { reactive, computed, nextTick, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useTestState } from '../composables/useTestState'
import { usePdfReport } from '../composables/usePdfReport'

const emit = defineEmits(['add-project'])

const {
  projects, runs, lines, openTabs, activeTab,
  runStatus, getRun, statusLabel, tagClass, fmtMs, fmtElapsed,
  remainCount, totalCount, classifyLine,
  startRun, stopRun, activateTab, closeTab, ensureTab,
} = useTestState()

const { exportReport } = usePdfReport({ runs, lines, runStatus, fmtElapsed })

// ── History state ────────────────────────────────────────────────────────────
const hist = reactive({ open: false, weeks: 1, runs: [], loading: false })

const filteredRuns = computed(() => {
  const cutoff = Date.now() - hist.weeks * 7 * 24 * 60 * 60 * 1000
  return hist.runs.filter(r => {
    const ts = new Date(r.startedAt.replace(' ', 'T')).getTime()
    return !isNaN(ts) && ts >= cutoff
  })
})

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

  if (runStatus(run.suiteId) === 'running') return

  runs[run.suiteId] = {
    status:     run.status,
    startTime:  null,
    elapsed:    0,
    duration:   run.durationMs,
    passCount:  run.passCount,
    failCount:  run.failCount,
    totalCount: run.totalCount ?? (run.passCount + run.failCount),
    stopping:   false,
    projectId:  run.projectId,
  }

  try {
    const outputLines = await invoke('get_run_output', { runId: run.id })
    lines[run.suiteId] = outputLines.map(text => ({ text, cls: classifyLine(text) }))

    // Re-derive pass/fail counts from output lines when DB values are missing
    // (old runs saved before the Rust counter fix have 0s in the DB)
    if (runs[run.suiteId] && runs[run.suiteId].passCount === 0 && runs[run.suiteId].failCount === 0) {
      let derivedPass = 0, derivedFail = 0
      for (const { text, cls } of lines[run.suiteId]) {
        if (cls === 'l-pass') derivedPass++
        else if (cls === 'l-fail') derivedFail++
        const mPass = text.match(/(\d+)\s+passed/i)
        if (mPass) { const n = parseInt(mPass[1]); if (n > derivedPass) derivedPass = n }
        const mFail = text.match(/(\d+)\s+failed/i)
        if (mFail) { const n = parseInt(mFail[1]); if (n > derivedFail) derivedFail = n }
      }
      if (derivedPass > 0 || derivedFail > 0) {
        runs[run.suiteId].passCount  = derivedPass
        runs[run.suiteId].failCount  = derivedFail
        runs[run.suiteId].totalCount = derivedPass + derivedFail
      }
    }
  } catch (e) {
    console.error('[history] erro ao carregar output:', e)
    lines[run.suiteId] = [{ text: '(output nao disponivel para esta execucao)', cls: 'l-default' }]
  }

  await nextTick()
  const el = document.getElementById(`term-${run.suiteId}`)
  if (el) el.scrollTop = el.scrollHeight
}

function formatRunDate(str) {
  const d = new Date(str.replace(' ', 'T'))
  if (isNaN(d)) return str
  const now    = new Date()
  const diffMs = now - d
  const diffD  = Math.floor(diffMs / 86400000)
  const time   = d.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' })
  if (diffD === 0) return `Hoje, ${time}`
  if (diffD === 1) return `Ontem, ${time}`
  if (diffD < 7)   return `${d.toLocaleDateString('pt-BR', { weekday: 'short' })}, ${time}`
  return `${d.toLocaleDateString('pt-BR', { day: '2-digit', month: '2-digit' })}, ${time}`
}

// ── Auto-scroll terminals on mount ───────────────────────────────────────────
onMounted(() => {
  nextTick(() => {
    for (const tab of openTabs.value) {
      const el = document.getElementById(`term-${tab.suiteId}`)
      if (el) el.scrollTop = el.scrollHeight
    }
  })
})
</script>
