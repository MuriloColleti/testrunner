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

const emit = defineEmits(['add-project'])

const {
  projects, runs, lines, openTabs, activeTab,
  runStatus, getRun, statusLabel, tagClass, fmtMs, fmtElapsed,
  remainCount, totalCount, classifyLine,
  startRun, stopRun, activateTab, closeTab, ensureTab,
} = useTestState()

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
    totalCount: run.passCount + run.failCount,
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

// ── Export PDF ───────────────────────────────────────────────────────────────
function pdfSafe(s) {
  return String(s)
    .replace(/[\u2713\u2714]/g, '+ ')
    .replace(/[\u2717\u2718]/g, '\xD7 ')
    .replace(/\u25cf/g, '* ')
    .replace(/[^\x00-\xFF]/g, '?')
}

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

function extractCoverageSection(out) {
  const result = []
  let inCoverage = false
  for (const line of out) {
    if (!inCoverage) {
      if (line.text && line.text.includes('Coverage report from')) {
        inCoverage = true
      }
    } else {
      result.push(line.text)
    }
  }
  return result
}

function parseCoverageRows(rawLines) {
  const rows = []
  for (const line of rawLines) {
    if (/^[\s\-+|]+$/.test(line)) continue
    const cells = line.split('|').map(c => c.trim())
    if (cells.length < 5 || !cells[0]) continue
    rows.push({
      file:      cells[0],
      stmts:     cells[1] ?? '',
      branch:    cells[2] ?? '',
      funcs:     cells[3] ?? '',
      lines:     cells[4] ?? '',
      uncovered: cells[5] ?? '',
    })
  }
  return rows
}

async function exportReport(suiteId, tab) {
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
    const covLines    = extractCoverageSection(out)
    const covRows     = parseCoverageRows(covLines)

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

    const doc = new jsPDF({ unit: 'pt', format: 'a4' })
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

    doc.setFont('helvetica', 'bold')
    doc.setFontSize(9)
    setT(stColor)
    doc.text(lbl, ML, y)
    y += 22

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

    if (fail > 0 && failedLines.length > 0) {
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

        if (isFailTitle && y > 200) y += 6
        if (isSumFail || isSumPass) y += 6

        const color  = isFailTitle ? RED : isError ? RED : isStack ? DIM
                     : isSumFail   ? RED : isSumPass ? GRN : MUTE
        const indent = isStack ? 20 : isError ? 10 : isContext ? 10 : 0

        const txt     = pdfSafe(line.text)
        const maxW    = CW - indent - 24
        const wrapped = doc.splitTextToSize(txt, maxW)

        wrapped.forEach((sl, wi) => {
          if (y + LH > BOT) newPage()
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
      setF(mix(GRN, 0.08))
      doc.rect(ML, y, CW, 40, 'F')
      setD(mix(GRN, 0.35))
      doc.setLineWidth(0.5)
      doc.rect(ML, y, CW, 40, 'D')
      doc.setFont('helvetica', 'bold')
      doc.setFontSize(11)
      setT(GRN)
      doc.text('Todos os testes passaram!', ML + 16, y + 24)
      y += 40
    }

    // ── Coverage table ──────────────────────────────────────────────────────
    if (covRows.length > 0) {
      y += 24

      doc.setFont('helvetica', 'bold')
      doc.setFontSize(8)
      setT(GRN)
      doc.text('COBERTURA DE C\xD3DIGO', ML, y)
      hr(y + 4, mix(GRN, 0.4))
      y += 18

      const COV_COLS = [CW * 0.38, CW * 0.12, CW * 0.12, CW * 0.12, CW * 0.12, CW * 0.14]
      const COV_HDR  = ['ARQUIVO', '% STMTS', '% BRANCH', '% FUNCS', '% LINES', 'DESCOBERTAS']
      const ROW_H    = 16

      const pctColor = str => {
        const n = parseFloat(str)
        if (isNaN(n)) return MUTE
        return n >= 80 ? GRN : n >= 60 ? AMB : RED
      }

      const addCovPage = () => { doc.addPage(); pageBg(); y = 40 }

      // Header row
      if (y + ROW_H > BOT - 10) addCovPage()
      setF(mix(MUTE, 0.15))
      doc.rect(ML, y, CW, ROW_H, 'F')
      doc.setFont('helvetica', 'bold')
      doc.setFontSize(7)
      let cx = ML + 8
      COV_HDR.forEach((lbl, i) => {
        setT(i === 0 ? MUTE : DIM)
        doc.text(lbl, cx, y + 10)
        cx += COV_COLS[i]
      })
      y += ROW_H

      // Data rows
      covRows.forEach((row, ri) => {
        if (y + ROW_H > BOT - 10) addCovPage()
        const isAllFiles = /^all files/i.test(row.file)
        setF(isAllFiles ? mix(GRN, 0.08) : ri % 2 === 0 ? CARD : mix(CARD, 0.7))
        doc.rect(ML, y, CW, ROW_H, 'F')

        cx = ML + 8

        // File name
        doc.setFont(isAllFiles ? 'helvetica' : 'courier', isAllFiles ? 'bold' : 'normal')
        doc.setFontSize(isAllFiles ? 7.5 : 7)
        setT(isAllFiles ? TEXT : MUTE)
        const fLabel  = pdfSafe(row.file)
        const maxFW   = COV_COLS[0] - 12
        const fWrapped = doc.splitTextToSize(fLabel, maxFW)
        doc.text(fWrapped[0], cx, y + 10)
        cx += COV_COLS[0]

        // Percentage columns
        doc.setFont('courier', isAllFiles ? 'bold' : 'normal')
        doc.setFontSize(7.5)
        ;[row.stmts, row.branch, row.funcs, row.lines].forEach((val, vi) => {
          setT(isAllFiles ? TEXT : pctColor(val))
          doc.text(String(val), cx + COV_COLS[vi + 1] - 16, y + 10, { align: 'right' })
          cx += COV_COLS[vi + 1]
        })

        // Uncovered line numbers
        doc.setFont('courier', 'normal')
        doc.setFontSize(6.5)
        setT(DIM)
        doc.text(pdfSafe(row.uncovered).slice(0, 28), cx + 2, y + 10)

        y += ROW_H
      })

      // Bottom rule
      setD(mix(GRN, 0.3))
      doc.setLineWidth(0.5)
      doc.line(ML, y, W - MR, y)
      y += 2
    }

    const pages = doc.internal.getNumberOfPages()
    for (let p = 1; p <= pages; p++) {
      doc.setPage(p)
      hr(BOT + 6)
      doc.setFont('helvetica', 'normal')
      doc.setFontSize(7)
      setT(DIM)
      doc.text('TestRunner', ML, BOT + 18)
      doc.text(
        `${pdfSafe(tab.projName)} \xB7 ${pdfSafe(tab.suiteName)} \xB7 ${date}   ${p}/${pages}`,
        W - MR, BOT + 18, { align: 'right' }
      )
    }

    const safe     = `${tab.projName}-${tab.suiteName}`.replace(/[^a-z0-9]/gi, '-').toLowerCase()
    const df       = now.toISOString().slice(0, 10)
    const filename = `report-${safe}-${df}.pdf`
    const dataUri  = doc.output('datauristring')
    const data     = dataUri.split(',')[1]

    await invoke('save_pdf', { filename, data })
  } catch (err) {
    console.error('[exportReport] ERRO:', err)
    alert(`Erro ao gerar PDF:\n${err?.message ?? err}`)
  }
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
