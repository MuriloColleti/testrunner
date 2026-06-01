<template>
  <div class="dash-toolbar">
    <div class="dash-title">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="16" height="16">
        <rect x="1" y="8" width="3" height="6"/><rect x="6" y="4" width="3" height="10"/><rect x="11" y="1" width="3" height="13"/>
      </svg>
      <span>Dashboard</span>
    </div>
    <div class="dash-filters">
      <select v-model="dash.projectFilter" class="dash-select">
        <option value="">Todos os projetos</option>
        <option v-for="p in projects" :key="p.id" :value="p.id">{{ p.name }}</option>
      </select>
      <button class="act-btn" @click="exportDashboardExcel" title="Exportar Excel">
        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
          <path d="M7 1v8M4 6l3 3 3-3" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M2 10v2a1 1 0 001 1h8a1 1 0 001-1v-2" stroke-linecap="round"/>
        </svg>
        Exportar Excel
      </button>
    </div>
  </div>

  <div class="dash-content" v-if="!dash.loading">
    <!-- Summary cards -->
    <div class="dash-cards">
      <div class="dash-card dc-blue">
        <div class="dc-head">
          <span class="dc-icon dc-icon-blue">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
              <path d="M2 12l3-4 3 2 4-5 2 3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </span>
          <span class="dc-lbl">Total Executados</span>
        </div>
        <span class="dc-num">{{ dashSummary.totalTests }}</span>
        <span class="dc-sub">em {{ dashComparisons.length }} suite{{ dashComparisons.length !== 1 ? 's' : '' }}</span>
      </div>

      <div class="dash-card dc-green">
        <div class="dc-head">
          <span class="dc-icon dc-icon-green">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
              <path d="M3 8.5l3.5 3.5 6.5-7" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </span>
          <span class="dc-lbl">Taxa de Aprovacao</span>
        </div>
        <div class="dc-rate-row">
          <span class="dc-num">{{ dashSummary.passRate }}<span class="dc-pct">%</span></span>
          <div class="dc-rate-ring">
            <svg viewBox="0 0 36 36" width="40" height="40">
              <circle cx="18" cy="18" r="15" fill="none" stroke="var(--border)" stroke-width="3"/>
              <circle cx="18" cy="18" r="15" fill="none"
                :stroke="dashSummary.passRate >= 80 ? 'var(--green)' : dashSummary.passRate >= 50 ? 'var(--amber)' : 'var(--red)'"
                stroke-width="3" stroke-linecap="round"
                :stroke-dasharray="`${dashSummary.passRate * 0.9425} 94.25`"
                transform="rotate(-90 18 18)"/>
            </svg>
          </div>
        </div>
      </div>

      <div class="dash-card dc-improve">
        <div class="dc-head">
          <span class="dc-icon dc-icon-green">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
              <path d="M8 12V4M5 7l3-3 3 3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </span>
          <span class="dc-lbl">Melhoraram</span>
        </div>
        <span class="dc-num">{{ dashSummary.improved }}</span>
        <span class="dc-sub">suite{{ dashSummary.improved !== 1 ? 's' : '' }} com menos falhas</span>
      </div>

      <div class="dash-card dc-regress">
        <div class="dc-head">
          <span class="dc-icon dc-icon-red">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
              <path d="M8 4v8M5 9l3 3 3-3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </span>
          <span class="dc-lbl">Pioraram</span>
        </div>
        <span class="dc-num">{{ dashSummary.regressed }}</span>
        <span class="dc-sub">suite{{ dashSummary.regressed !== 1 ? 's' : '' }} com mais falhas</span>
      </div>

      <div v-if="dashSummary.avgUnitCoverage !== null" class="dash-card dc-cov">
        <div class="dc-head">
          <span class="dc-icon dc-icon-blue">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
              <path d="M2 12l3-3 2 2 4-5 3 2" stroke-linecap="round" stroke-linejoin="round"/>
              <rect x="1" y="13" width="14" height="1.5" rx=".75" fill="currentColor" stroke="none"/>
            </svg>
          </span>
          <span class="dc-lbl">Cobertura Unit</span>
        </div>
        <div class="dc-rate-row">
          <span class="dc-num" :style="{ color: dashSummary.avgUnitCoverage >= 80 ? 'var(--green)' : dashSummary.avgUnitCoverage >= 50 ? 'var(--amber)' : 'var(--red)' }">
            {{ dashSummary.avgUnitCoverage }}<span class="dc-pct">%</span>
          </span>
        </div>
        <span class="dc-sub">em {{ dashSummary.unitCovSuitesCount }} suite{{ dashSummary.unitCovSuitesCount !== 1 ? 's' : '' }}</span>
      </div>

      <div v-if="dashSummary.avgE2eCoverage !== null" class="dash-card dc-cov dc-cov-e2e">
        <div class="dc-head">
          <span class="dc-icon dc-icon-purple">
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="14" height="14">
              <path d="M2 12l3-3 2 2 4-5 3 2" stroke-linecap="round" stroke-linejoin="round"/>
              <rect x="1" y="13" width="14" height="1.5" rx=".75" fill="currentColor" stroke="none"/>
            </svg>
          </span>
          <span class="dc-lbl">Cobertura E2E</span>
        </div>
        <div class="dc-rate-row">
          <span class="dc-num" :style="{ color: dashSummary.avgE2eCoverage >= 80 ? 'var(--green)' : dashSummary.avgE2eCoverage >= 50 ? 'var(--amber)' : 'var(--red)' }">
            {{ dashSummary.avgE2eCoverage }}<span class="dc-pct">%</span>
          </span>
        </div>
        <span class="dc-sub">em {{ dashSummary.e2eCovSuitesCount }} suite{{ dashSummary.e2eCovSuitesCount !== 1 ? 's' : '' }}</span>
      </div>
    </div>

    <!-- Failure comparison chart -->
    <div class="dash-chart" v-if="sortedComparisons.length > 0">
      <div class="chart-hd">
        <span class="chart-title">Falhas por Suite: Anterior vs Atual</span>
        <div class="chart-legend">
          <span class="legend-item"><span class="legend-dot lg-prev"></span>Anterior</span>
          <span class="legend-item"><span class="legend-dot lg-curr"></span>Atual</span>
        </div>
      </div>

      <!-- Grid axis labels -->
      <div class="chart-axis">
        <div class="axis-label-area"></div>
        <div class="axis-ticks">
          <span class="axis-tick" style="left:0">0</span>
          <span class="axis-tick" style="left:50%">{{ Math.round(dashMaxFail / 2) }}</span>
          <span class="axis-tick" style="left:100%">{{ dashMaxFail }}</span>
        </div>
        <div class="axis-delta-area"></div>
        <div class="axis-rate-area">Aprovacao</div>
        <div class="axis-cov-area">Cobertura</div>
      </div>

      <div class="chart-body">
        <div
          v-for="comp in sortedComparisons"
          :key="comp.suiteId"
          class="chart-row"
          :class="{ 'row-improved': comp.delta < 0, 'row-regressed': comp.delta > 0 }"
        >
          <!-- Label -->
          <div class="chart-label">
            <div class="cl-top">
              <span class="chart-suite">{{ comp.suiteName }}</span>
              <span class="tag sm" :class="tagClass(comp.suiteTag)">{{ comp.suiteTag }}</span>
            </div>
            <span class="chart-proj">{{ comp.projectName }}</span>
          </div>

          <!-- Bars area with grid -->
          <div class="chart-bars">
            <div class="bars-grid">
              <div class="grid-line" style="left:50%"></div>
            </div>
            <div class="bar-group">
              <div class="bar-line">
                <div class="bar bar-prev" :style="{ width: barWidth(comp.previous?.failCount || 0) }"></div>
                <span class="bar-num prev">{{ comp.previous?.failCount ?? '-' }}</span>
              </div>
              <div class="bar-line">
                <div class="bar bar-curr" :class="{ zero: comp.current.failCount === 0 }" :style="{ width: barWidth(comp.current.failCount) }"></div>
                <span class="bar-num curr">{{ comp.current.failCount }}</span>
              </div>
            </div>
          </div>

          <!-- Delta pill -->
          <div class="chart-delta-wrap">
            <span v-if="comp.delta !== 0" class="delta-pill" :class="comp.delta < 0 ? 'dp-green' : 'dp-red'">
              <svg v-if="comp.delta < 0" viewBox="0 0 10 10" fill="currentColor" width="8" height="8">
                <path d="M5 2L1 7h8L5 2z"/>
              </svg>
              <svg v-else viewBox="0 0 10 10" fill="currentColor" width="8" height="8">
                <path d="M5 8L1 3h8L5 8z"/>
              </svg>
              {{ Math.abs(comp.delta) }}
            </span>
            <span v-else class="delta-pill dp-neutral">=</span>
          </div>

          <!-- Pass rate mini bar -->
          <div class="chart-rate">
            <div class="rate-bar">
              <div class="rate-pass" :style="{ width: passRate(comp) + '%' }"></div>
            </div>
            <span class="rate-pct">{{ passRate(comp) }}%</span>
          </div>

          <!-- Coverage -->
          <div class="chart-cov">
            <span v-if="comp.current.coveragePct !== null"
              class="cov-pct"
              :class="comp.current.coveragePct >= 80 ? 'cov-good' : comp.current.coveragePct >= 50 ? 'cov-mid' : 'cov-bad'">
              {{ Math.round(comp.current.coveragePct) }}%
            </span>
            <span v-else class="cov-na">—</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Empty state -->
    <div v-else class="dash-empty">
      <div class="dash-empty-icon">
        <svg viewBox="0 0 48 48" fill="none" stroke="currentColor" stroke-width="1.5" width="40" height="40">
          <rect x="6" y="26" width="8" height="14" rx="1"/><rect x="20" y="18" width="8" height="22" rx="1"/><rect x="34" y="10" width="8" height="30" rx="1"/>
        </svg>
      </div>
      <p>Sem dados de execucao</p>
      <span>Execute testes para visualizar o dashboard</span>
    </div>
  </div>

  <div v-else class="dash-loading">
    <div class="loading-spinner"></div>
    <span>Carregando dados...</span>
  </div>
</template>

<script setup>
import { reactive, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import ExcelJS from 'exceljs'
import { useTestState } from '../composables/useTestState'

const { projects, tagClass } = useTestState()

// ── State ────────────────────────────────────────────────────────────────────
const dash = reactive({ projectFilter: '', loading: false, runs: [] })

// ── Computed ─────────────────────────────────────────────────────────────────
const dashComparisons = computed(() => {
  let runs = dash.runs
  if (dash.projectFilter) {
    runs = runs.filter(r => r.projectId === dash.projectFilter)
  }
  const grouped = {}
  for (const run of runs) {
    if (!grouped[run.suiteId]) grouped[run.suiteId] = []
    if (grouped[run.suiteId].length < 2) grouped[run.suiteId].push(run)
  }
  return Object.entries(grouped).map(([suiteId, suiteRuns]) => {
    const current = suiteRuns[0]
    const previous = suiteRuns[1] || null
    return {
      suiteId,
      suiteName: current.suiteName,
      suiteTag: current.suiteTag,
      projectName: current.projectName,
      current: { failCount: current.failCount, passCount: current.passCount, status: current.status, date: current.startedAt, coveragePct: current.coveragePct ?? null },
      previous: previous ? { failCount: previous.failCount, passCount: previous.passCount, status: previous.status, date: previous.startedAt, coveragePct: previous.coveragePct ?? null } : null,
      delta: previous ? current.failCount - previous.failCount : 0,
    }
  })
})

const sortedComparisons = computed(() => {
  return [...dashComparisons.value].sort((a, b) => b.delta - a.delta)
})

const dashSummary = computed(() => {
  const comps = dashComparisons.value
  const totalPass  = comps.reduce((s, c) => s + c.current.passCount, 0)
  const totalFail  = comps.reduce((s, c) => s + c.current.failCount, 0)
  const totalTests = totalPass + totalFail
  const unitSuites = comps.filter(c => c.current.coveragePct !== null && c.suiteTag?.toUpperCase() === 'UNIT')
  const e2eSuites  = comps.filter(c => c.current.coveragePct !== null && c.suiteTag?.toUpperCase() === 'E2E')
  const avgUnit = unitSuites.length > 0
    ? Math.round(unitSuites.reduce((s, c) => s + c.current.coveragePct, 0) / unitSuites.length)
    : null
  const avgE2e  = e2eSuites.length > 0
    ? Math.round(e2eSuites.reduce((s, c) => s + c.current.coveragePct, 0) / e2eSuites.length)
    : null
  return {
    totalTests,
    totalCurrentPass:  totalPass,
    totalCurrentFails: totalFail,
    passRate: totalTests > 0 ? Math.round((totalPass / totalTests) * 100) : 0,
    improved:  comps.filter(c => c.delta < 0).length,
    regressed: comps.filter(c => c.delta > 0).length,
    avgUnitCoverage:    avgUnit,
    unitCovSuitesCount: unitSuites.length,
    avgE2eCoverage:     avgE2e,
    e2eCovSuitesCount:  e2eSuites.length,
  }
})

const dashMaxFail = computed(() => {
  let max = 1
  for (const c of dashComparisons.value) {
    if (c.current.failCount > max) max = c.current.failCount
    if (c.previous && c.previous.failCount > max) max = c.previous.failCount
  }
  return max
})

// ── Functions ────────────────────────────────────────────────────────────────
function barWidth(val) {
  if (!val || dashMaxFail.value === 0) return '0%'
  return (val / dashMaxFail.value) * 100 + '%'
}

function passRate(comp) {
  const total = comp.current.passCount + comp.current.failCount
  if (total === 0) return 0
  return Math.round((comp.current.passCount / total) * 100)
}

async function loadDashboard() {
  dash.loading = true
  try {
    dash.runs = await invoke('get_runs', { projectId: null })
  } catch (e) {
    console.error('[dashboard]', e)
  } finally {
    dash.loading = false
  }
}

async function exportDashboardExcel() {
  const comps = sortedComparisons.value
  if (!comps.length) return
  const summary = dashSummary.value
  const date = new Date().toLocaleDateString('pt-BR')

  // ── Dark theme palette (matches dashboard CSS variables) ───────────────────
  const P = {
    bgDeep: '010409', bg: '0D1117', bgCard: '161B22',
    border: '21262D', borderHi: '30363D',
    text: 'E6EDF3', muted: '8B949E', dim: '484F58',
    green: '3FB950', red: 'F85149', amber: 'D29922',
    blue: '58A6FF', purple: 'BC8CFF',
    greenBg: '12261A', redBg: '2D1215', blueBg: '0D1D2E',
    amberBg: '2A2010', purpleBg: '1F1530',
  }

  const _fill = c => ({ type: 'pattern', pattern: 'solid', fgColor: { argb: 'FF' + c }, bgColor: { argb: 'FF' + c } })
  const _fn   = (c, o = {}) => ({ color: { argb: 'FF' + c }, name: 'Segoe UI', size: 10, ...o })
  const _mono = (c, o = {}) => ({ color: { argb: 'FF' + c }, name: 'Consolas',  size: 10, ...o })
  const _bdr  = (c = P.border, s = 'thin') => ({ style: s, color: { argb: 'FF' + c } })

  const wb = new ExcelJS.Workbook()
  wb.creator = 'TestRunner'
  wb.created = new Date()

  const NC = 10

  // Helper: fill entire row with bg
  const fillRow = (ws, rn, bg, h) => {
    const r = ws.getRow(rn)
    if (h) r.height = h
    for (let c = 1; c <= NC; c++) r.getCell(c).fill = _fill(bg)
    return r
  }

  // ════════════════════════════════════════════════════════════════════════════
  //  Sheet: Dashboard
  // ════════════════════════════════════════════════════════════════════════════
  const ws = wb.addWorksheet('Dashboard', {
    views: [{ state: 'frozen', ySplit: 8, showGridLines: false }],
    properties: { tabColor: { argb: 'FF' + P.blue } },
  })

  ws.columns = [
    { width: 20 }, { width: 24 }, { width: 10 },
    { width: 14 }, { width: 14 }, { width: 11 },
    { width: 14 }, { width: 14 }, { width: 13 }, { width: 13 },
  ]

  // ── Row 1: Title bar ───────────────────────────────────────────────────────
  fillRow(ws, 1, P.bgCard, 42)
  ws.mergeCells('A1:I1')
  const tc = ws.getCell('A1')
  tc.value = `   TestRunner  \u00b7  Dashboard  \u00b7  ${date}`
  tc.font = { bold: true, size: 14, color: { argb: 'FF' + P.text }, name: 'Segoe UI' }
  tc.alignment = { vertical: 'middle' }
  tc.border = { bottom: { style: 'medium', color: { argb: 'FF' + P.blue } } }

  // ── Row 2: Spacer ──────────────────────────────────────────────────────────
  fillRow(ws, 2, P.bg, 10)

  // ── Rows 3-5: Summary cards ────────────────────────────────────────────────
  // Card layout: A-B | C-E | F-G | H-I
  const cards = [
    { lbl: 'TOTAL EXECUTADOS',  val: String(summary.totalTests), sub: `em ${comps.length} suite(s)`,                                         accent: P.blue,  merge: [1, 2] },
    { lbl: 'TAXA DE APROVAÇÃO', val: `${summary.passRate}%`,     sub: `${summary.totalCurrentPass} passou \u00b7 ${summary.totalCurrentFails} falhou`, accent: summary.passRate >= 80 ? P.green : summary.passRate >= 50 ? P.amber : P.red, merge: [3, 5] },
    { lbl: 'MELHORARAM',        val: String(summary.improved),   sub: 'suite(s) com menos falhas',                                           accent: P.green, merge: [6, 7] },
    { lbl: 'PIORARAM',          val: String(summary.regressed),  sub: 'suite(s) com mais falhas',                                            accent: P.red,   merge: [8, 9] },
  ]

  for (const rn of [3, 4, 5]) fillRow(ws, rn, P.bg)
  ws.getRow(3).height = 24
  ws.getRow(4).height = 38
  ws.getRow(5).height = 22

  for (const cd of cards) {
    const [c1, c2] = cd.merge
    for (const rn of [3, 4, 5]) ws.mergeCells(rn, c1, rn, c2)

    // Label row
    const lbl = ws.getRow(3).getCell(c1)
    lbl.value = cd.lbl
    lbl.fill = _fill(P.bgCard)
    lbl.font = _fn(P.dim, { size: 9, bold: true })
    lbl.alignment = { vertical: 'middle', horizontal: 'left', indent: 1 }
    lbl.border = { top: { style: 'medium', color: { argb: 'FF' + cd.accent } }, left: _bdr(P.border), right: _bdr(P.border) }

    // Value row
    const val = ws.getRow(4).getCell(c1)
    val.value = cd.val
    val.fill = _fill(P.bgCard)
    val.font = { bold: true, size: 24, color: { argb: 'FF' + P.text }, name: 'Consolas' }
    val.alignment = { vertical: 'middle', horizontal: 'left', indent: 1 }
    val.border = { left: _bdr(P.border), right: _bdr(P.border) }

    // Sub row
    const sub = ws.getRow(5).getCell(c1)
    sub.value = cd.sub
    sub.fill = _fill(P.bgCard)
    sub.font = _fn(P.dim, { size: 9 })
    sub.alignment = { vertical: 'top', horizontal: 'left', indent: 1 }
    sub.border = { bottom: _bdr(P.border), left: _bdr(P.border), right: _bdr(P.border) }
  }

  // ── Row 6: Spacer ──────────────────────────────────────────────────────────
  fillRow(ws, 6, P.bg, 10)

  // ── Row 7: Chart title + legend ────────────────────────────────────────────
  fillRow(ws, 7, P.bgCard, 32)
  ws.mergeCells('A7:D7')
  const ct = ws.getCell('A7')
  ct.value = '   Falhas por Suite: Anterior vs Atual'
  ct.font = _fn(P.text, { size: 12, bold: true })
  ct.alignment = { vertical: 'middle' }
  ct.border = { bottom: _bdr(P.border) }

  ws.mergeCells('E7:G7')
  const legP = ws.getCell('E7')
  legP.value = '\u25a0 Anterior'
  legP.font = _fn(P.dim)
  legP.alignment = { vertical: 'middle', horizontal: 'right' }
  legP.border = { bottom: _bdr(P.border) }

  ws.mergeCells('H7:I7')
  const legC = ws.getCell('H7')
  legC.value = '\u25a0 Atual'
  legC.font = _fn(P.red)
  legC.alignment = { vertical: 'middle', horizontal: 'right' }
  legC.border = { bottom: _bdr(P.border) }

  // ── Row 8: Column headers ──────────────────────────────────────────────────
  fillRow(ws, 8, P.bgDeep, 28)
  const hdrs = ['Projeto', 'Suite', 'Tag', 'Falhas Ant.', 'Falhas Atual', 'Delta', 'Passou Ant.', 'Passou Atual', 'Aprovação', 'Cobertura']
  hdrs.forEach((h, i) => {
    const cell = ws.getRow(8).getCell(i + 1)
    cell.value = h
    cell.font = _fn(P.muted, { bold: true })
    cell.alignment = { vertical: 'middle', horizontal: i >= 3 ? 'center' : 'left' }
    cell.border = { bottom: _bdr(P.blue, 'medium'), top: _bdr(P.borderHi) }
  })

  // ── Data rows ──────────────────────────────────────────────────────────────
  comps.forEach((c, idx) => {
    const rn = idx + 9
    const rowBg = idx % 2 === 0 ? P.bg : P.bgCard
    const r = fillRow(ws, rn, rowBg, 30)

    // Bottom border + left accent
    for (let col = 1; col <= NC; col++) {
      r.getCell(col).border = { bottom: _bdr(P.border) }
    }
    if (c.delta > 0) {
      r.getCell(1).border = { left: _bdr(P.red, 'medium'), bottom: _bdr(P.border) }
    } else if (c.delta < 0) {
      r.getCell(1).border = { left: _bdr(P.green, 'medium'), bottom: _bdr(P.border) }
    }

    // A: Projeto
    r.getCell(1).value = c.projectName
    r.getCell(1).font = _fn(P.muted)
    r.getCell(1).alignment = { vertical: 'middle' }

    // B: Suite
    r.getCell(2).value = c.suiteName
    r.getCell(2).font = _fn(P.text, { bold: true })
    r.getCell(2).alignment = { vertical: 'middle' }

    // C: Tag (pill style)
    const tagCell = r.getCell(3)
    tagCell.value = c.suiteTag
    tagCell.alignment = { vertical: 'middle', horizontal: 'center' }
    if (c.suiteTag === 'E2E') {
      tagCell.fill = _fill(P.blueBg)
      tagCell.font = _fn(P.blue, { bold: true, size: 9 })
    } else if (c.suiteTag === 'API') {
      tagCell.fill = _fill(P.purpleBg)
      tagCell.font = _fn(P.purple, { bold: true, size: 9 })
    } else {
      tagCell.fill = _fill(P.greenBg)
      tagCell.font = _fn(P.green, { bold: true, size: 9 })
    }

    // D: Falhas Ant.
    r.getCell(4).value = c.previous?.failCount ?? '\u2014'
    r.getCell(4).font = _mono(P.dim)
    r.getCell(4).alignment = { vertical: 'middle', horizontal: 'center' }

    // E: Falhas Atual
    r.getCell(5).value = c.current.failCount
    r.getCell(5).alignment = { vertical: 'middle', horizontal: 'center' }
    r.getCell(5).font = c.current.failCount > 0
      ? _mono(P.red, { bold: true, size: 11 })
      : _mono(P.green, { bold: true, size: 11 })

    // F: Delta (arrow + colored bg)
    const dc = r.getCell(6)
    dc.alignment = { vertical: 'middle', horizontal: 'center' }
    if (c.delta < 0) {
      dc.value = `\u25b2 ${Math.abs(c.delta)}`
      dc.fill = _fill(P.greenBg)
      dc.font = _mono(P.green, { bold: true })
    } else if (c.delta > 0) {
      dc.value = `\u25bc ${c.delta}`
      dc.fill = _fill(P.redBg)
      dc.font = _mono(P.red, { bold: true })
    } else {
      dc.value = '='
      dc.font = _mono(P.dim)
    }

    // G: Passou Ant.
    r.getCell(7).value = c.previous?.passCount ?? '\u2014'
    r.getCell(7).font = _mono(P.dim)
    r.getCell(7).alignment = { vertical: 'middle', horizontal: 'center' }

    // H: Passou Atual
    r.getCell(8).value = c.current.passCount
    r.getCell(8).font = _mono(P.green)
    r.getCell(8).alignment = { vertical: 'middle', horizontal: 'center' }

    // I: Aprovação %
    const rc = r.getCell(9)
    const rv = passRate(c)
    rc.value = rv / 100
    rc.numFmt = '0%'
    rc.alignment = { vertical: 'middle', horizontal: 'center' }
    if (rv >= 90)      rc.font = _mono(P.green, { bold: true })
    else if (rv >= 70) rc.font = _mono(P.amber, { bold: true })
    else               rc.font = _mono(P.red, { bold: true })

    // J: Cobertura %
    const cc = r.getCell(10)
    cc.alignment = { vertical: 'middle', horizontal: 'center' }
    if (c.current.coveragePct !== null) {
      cc.value = c.current.coveragePct / 100
      cc.numFmt = '0.0%'
      const cv = c.current.coveragePct
      cc.font = cv >= 80 ? _mono(P.green, { bold: true }) : cv >= 50 ? _mono(P.amber, { bold: true }) : _mono(P.red, { bold: true })
    } else {
      cc.value = '\u2014'
      cc.font = _mono(P.dim)
    }
  })

  // ── Total row ──────────────────────────────────────────────────────────────
  const trn = comps.length + 9
  const tr = fillRow(ws, trn, P.bgDeep, 30)

  for (let c = 1; c <= NC; c++) {
    tr.getCell(c).border = { top: _bdr(P.blue, 'medium') }
    tr.getCell(c).font = _mono(P.text, { bold: true })
    tr.getCell(c).alignment = { vertical: 'middle', horizontal: 'center' }
  }

  ws.mergeCells(trn, 1, trn, 3)
  tr.getCell(1).value = '   TOTAL'
  tr.getCell(1).font = _fn(P.text, { bold: true, size: 11 })
  tr.getCell(1).alignment = { vertical: 'middle', horizontal: 'left' }

  const tfp = comps.reduce((s, c) => s + (c.previous?.failCount || 0), 0)
  const tfc = summary.totalCurrentFails
  const td  = tfc - tfp
  const tpp = comps.reduce((s, c) => s + (c.previous?.passCount || 0), 0)
  const tpc = summary.totalCurrentPass
  const trt = summary.totalTests > 0 ? tpc / summary.totalTests : 0

  tr.getCell(4).value = tfp;  tr.getCell(4).font = _mono(P.dim, { bold: true })
  tr.getCell(5).value = tfc;  tr.getCell(5).font = tfc > 0 ? _mono(P.red, { bold: true }) : _mono(P.green, { bold: true })
  tr.getCell(6).value = td;   tr.getCell(6).font = td < 0 ? _mono(P.green, { bold: true }) : td > 0 ? _mono(P.red, { bold: true }) : _mono(P.dim, { bold: true })
  tr.getCell(7).value = tpp;  tr.getCell(7).font = _mono(P.dim, { bold: true })
  tr.getCell(8).value = tpc;  tr.getCell(8).font = _mono(P.green, { bold: true })
  tr.getCell(9).value = trt;  tr.getCell(9).numFmt = '0%'
  const covSuitesXl = comps.filter(c => c.current.coveragePct !== null)
  if (covSuitesXl.length > 0) {
    const avgCov = covSuitesXl.reduce((s, c) => s + c.current.coveragePct, 0) / covSuitesXl.length / 100
    tr.getCell(10).value = avgCov
    tr.getCell(10).numFmt = '0.0%'
    tr.getCell(10).font = avgCov >= 0.8 ? _mono(P.green, { bold: true }) : avgCov >= 0.5 ? _mono(P.amber, { bold: true }) : _mono(P.red, { bold: true })
  } else {
    tr.getCell(10).value = '\u2014'
    tr.getCell(10).font = _mono(P.dim)
  }

  // Autofilter on data columns
  ws.autoFilter = { from: { row: 8, column: 1 }, to: { row: comps.length + 8, column: NC } }

  // ════════════════════════════════════════════════════════════════════════════
  //  Sheet: Resumo
  // ════════════════════════════════════════════════════════════════════════════
  const wsR = wb.addWorksheet('Resumo', {
    views: [{ showGridLines: false }],
    properties: { tabColor: { argb: 'FF' + P.green } },
  })
  wsR.columns = [{ width: 28 }, { width: 18 }]

  // Title
  wsR.mergeCells('A1:B1')
  const rt = wsR.getCell('A1')
  rt.value = `   Resumo  \u00b7  ${date}`
  rt.fill = _fill(P.bgCard)
  rt.font = { bold: true, size: 14, color: { argb: 'FF' + P.text }, name: 'Segoe UI' }
  rt.alignment = { vertical: 'middle' }
  rt.border = { bottom: { style: 'medium', color: { argb: 'FF' + P.blue } } }
  wsR.getRow(1).height = 42

  const resumo = [
    { m: 'Total de Testes',    v: summary.totalTests,        c: P.text },
    { m: 'Passou (atual)',     v: summary.totalCurrentPass,  c: P.green },
    { m: 'Falhou (atual)',     v: summary.totalCurrentFails, c: P.red },
    { m: 'Taxa de Aprovação',  v: summary.passRate / 100,    c: summary.passRate >= 80 ? P.green : summary.passRate >= 50 ? P.amber : P.red, fmt: '0%' },
    { m: 'Suites Melhoraram',  v: summary.improved,          c: P.green },
    { m: 'Suites Pioraram',    v: summary.regressed,         c: P.red },
    { m: 'Total de Suites',    v: comps.length,              c: P.text },
    ...(summary.avgUnitCoverage !== null ? [{ m: 'Cobertura Unit (média)', v: summary.avgUnitCoverage / 100, c: summary.avgUnitCoverage >= 80 ? P.green : summary.avgUnitCoverage >= 50 ? P.amber : P.red, fmt: '0%' }] : []),
    ...(summary.avgE2eCoverage  !== null ? [{ m: 'Cobertura E2E (média)',  v: summary.avgE2eCoverage  / 100, c: summary.avgE2eCoverage  >= 80 ? P.green : summary.avgE2eCoverage  >= 50 ? P.amber : P.red, fmt: '0%' }] : []),
  ]

  resumo.forEach((item, idx) => {
    const rn = idx + 2
    const r = wsR.getRow(rn)
    r.height = 32
    const bg = idx % 2 === 0 ? P.bg : P.bgCard

    const m = r.getCell(1)
    m.value = item.m
    m.fill = _fill(bg)
    m.font = _fn(P.muted, { bold: true, size: 11 })
    m.alignment = { vertical: 'middle', indent: 1 }
    m.border = { bottom: _bdr(P.border) }

    const v = r.getCell(2)
    v.value = item.v
    v.fill = _fill(bg)
    v.font = _mono(item.c, { bold: true, size: 14 })
    v.alignment = { vertical: 'middle', horizontal: 'center' }
    v.border = { bottom: _bdr(P.border) }
    if (item.fmt) v.numFmt = item.fmt
  })

  // ════════════════════════════════════════════════════════════════════════════
  //  Save
  // ════════════════════════════════════════════════════════════════════════════
  const buffer = await wb.xlsx.writeBuffer()
  const filename = `dashboard-${new Date().toISOString().slice(0, 10)}.xlsx`
  try {
    await invoke('save_pdf', { filename, data: Array.from(new Uint8Array(buffer)) })
  } catch (e) {
    if (e !== 'cancelado') console.error('[export xlsx]', e)
  }
}

// ── Boot ─────────────────────────────────────────────────────────────────────
onMounted(() => loadDashboard())
</script>
