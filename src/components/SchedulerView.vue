<template>
  <div class="scheduler-view">

    <!-- Top bar -->
    <div class="sv-topbar">
      <h2 class="sv-title">Agendamentos</h2>
      <button class="btn-primary sv-new-btn" @click="openNew">
        <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="2" width="11" height="11">
          <path d="M6 1v10M1 6h10"/>
        </svg>
        Novo Agendamento
      </button>
    </div>

    <!-- Content -->
    <div class="sv-content">

      <!-- Calendário -->
      <div class="sv-aside">
        <CalendarMonth
          :schedules="schedules"
          :selected-date="selectedDate"
          @day-click="onDayClick"
        />
        <button
          v-if="selectedDate"
          class="sv-clear-filter"
          @click="selectedDate = ''"
        >
          <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.8" width="9" height="9">
            <path d="M2 2l6 6M8 2l-6 6" stroke-linecap="round"/>
          </svg>
          Limpar filtro
        </button>
      </div>

      <!-- Lista de agendamentos -->
      <div class="sv-list-col">
        <div class="sv-list-header">
          <span class="sv-list-title">
            {{ selectedDate ? fmtSelectedDate(selectedDate) : 'Todos os agendamentos' }}
          </span>
          <span class="sv-list-count">{{ filteredSchedules.length }}</span>
        </div>

        <div v-if="filteredSchedules.length === 0" class="sv-empty">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" width="32" height="32" style="color:var(--dim)">
            <rect x="3" y="4" width="18" height="18" rx="2"/><path d="M16 2v4M8 2v4M3 10h18"/>
          </svg>
          <span>{{ selectedDate ? 'Nenhum agendamento neste dia.' : 'Nenhum agendamento criado ainda.' }}</span>
        </div>

        <div v-else class="sv-list">
          <div
            v-for="s in filteredSchedules"
            :key="s.id"
            class="sched-card"
            :class="{ 'is-disabled': !s.enabled }"
          >
            <!-- Left: info -->
            <div class="sched-info">
              <div class="sched-label">{{ s.label }}</div>
              <div class="sched-meta">
                <span class="sched-proj">{{ projName(s.projectId) }}</span>
                <span class="sched-sep">·</span>
                <span class="sched-suite">{{ suiteName(s.projectId, s.suiteId) }}</span>
              </div>
              <div class="sched-when">
                <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" width="11" height="11">
                  <circle cx="6" cy="6" r="5"/><path d="M6 3v3l2 1.5" stroke-linecap="round"/>
                </svg>
                {{ fmtScheduledAt(s.scheduledAt) }}
                <span class="sched-rec" :class="`rec-${s.recurrence}`">{{ recLabel(s.recurrence) }}</span>
              </div>
            </div>

            <!-- Right: actions -->
            <div class="sched-actions">
              <!-- Toggle enabled -->
              <button
                class="sched-toggle"
                :class="{ on: s.enabled }"
                :title="s.enabled ? 'Desativar' : 'Ativar'"
                @click="handleToggle(s)"
              >
                <span class="toggle-track">
                  <span class="toggle-thumb"></span>
                </span>
              </button>
              <!-- Edit -->
              <button class="sched-icon-btn" title="Editar" @click="openEdit(s)">
                <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" width="12" height="12">
                  <path d="M9.5 1.5l3 3L4 13H1v-3L9.5 1.5z"/>
                </svg>
              </button>
              <!-- Delete -->
              <button class="sched-icon-btn danger" title="Remover" @click="handleDelete(s)">
                <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" width="12" height="12">
                  <path d="M2 4h10M5 4V2h4v2M5.5 7v4M8.5 7v4"/><path d="M3 4l.8 8h6.4L11 4"/>
                </svg>
              </button>
            </div>
          </div>
        </div>
      </div><!-- /sv-list-col -->

    </div><!-- /sv-content -->

    <!-- Modal de criação / edição -->
    <Transition name="modal">
      <ScheduleModal
        v-if="modal.open"
        :edit-schedule="modal.editSchedule"
        @close="modal.open = false"
      />
    </Transition>

  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { useTestState }  from '../composables/useTestState'
import { useScheduler }  from '../composables/useScheduler'
import CalendarMonth     from './CalendarMonth.vue'
import ScheduleModal     from './ScheduleModal.vue'

// ── External state ────────────────────────────────────────────────────────────
const { projects }                          = useTestState()
const { schedules, loadSchedules,
        deleteSchedule, toggleSchedule }    = useScheduler()

// ── Local state ───────────────────────────────────────────────────────────────
const selectedDate = ref('')

const modal = reactive({
  open:         false,
  editSchedule: null,
})

// ── Boot ──────────────────────────────────────────────────────────────────────
onMounted(loadSchedules)

// ── Computed ──────────────────────────────────────────────────────────────────
const filteredSchedules = computed(() => {
  if (!selectedDate.value) return schedules.value
  return schedules.value.filter(s => s.scheduledAt?.startsWith(selectedDate.value))
})

// ── Calendar handlers ─────────────────────────────────────────────────────────
function onDayClick(dateStr) {
  selectedDate.value = selectedDate.value === dateStr ? '' : dateStr
}

// ── Modal ─────────────────────────────────────────────────────────────────────
function openNew()   { modal.editSchedule = null; modal.open = true }
function openEdit(s) { modal.editSchedule = s;    modal.open = true }

// ── Actions ───────────────────────────────────────────────────────────────────
async function handleToggle(s) {
  await toggleSchedule(s.id, !s.enabled)
}

async function handleDelete(s) {
  if (!confirm(`Remover "${s.label}"?`)) return
  await deleteSchedule(s.id)
}

// ── Helpers ───────────────────────────────────────────────────────────────────
function projName(projectId) {
  return projects.value.find(p => p.id === projectId)?.name ?? projectId
}

function suiteName(projectId, suiteId) {
  const proj = projects.value.find(p => p.id === projectId)
  const s    = proj?.suites.find(s => s.id === suiteId)
  return s ? `${s.system} · ${s.name}` : suiteId
}

const REC_LABELS = { once: 'Uma vez', daily: 'Diário', weekly: 'Semanal' }
function recLabel(r) { return REC_LABELS[r] ?? r }

function fmtScheduledAt(dt) {
  if (!dt) return '—'
  // SQLite datetime() usa espaço; formato original usa T — normaliza os dois
  const sep = dt.includes('T') ? 'T' : ' '
  const [datePart, timePart] = dt.split(sep)
  const parts = (datePart ?? '').split('-')
  const [y, m, d] = parts
  const time = (timePart ?? '').slice(0, 5) || '??:??'
  return `${d}/${m}/${y} às ${time}`
}

const MONTHS_SHORT = ['Jan','Fev','Mar','Abr','Mai','Jun','Jul','Ago','Set','Out','Nov','Dez']
function fmtSelectedDate(ds) {
  const [y, m, d] = ds.split('-')
  return `${parseInt(d)} de ${MONTHS_SHORT[parseInt(m) - 1]} de ${y}`
}
</script>

<style scoped>
/* ── Layout ──────────────────────────────────────────────────────────────────── */
.scheduler-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background: var(--bg);
}

/* Top bar */
.sv-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
.sv-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}
.sv-new-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  padding: 6px 12px;
}

/* Content: two-column */
.sv-content {
  display: grid;
  grid-template-columns: 260px 1fr;
  gap: 20px;
  padding: 20px;
  overflow: hidden;
  flex: 1;
  min-height: 0;
}

/* Left col (calendar) */
.sv-aside {
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-self: start;
}
.sv-clear-filter {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--muted);
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 4px 2px;
  transition: color .12s;
}
.sv-clear-filter:hover { color: var(--text); }

/* Right col (list) */
.sv-list-col {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}
.sv-list-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
  flex-shrink: 0;
}
.sv-list-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--muted);
  text-transform: uppercase;
  letter-spacing: .05em;
}
.sv-list-count {
  font-size: 10px;
  color: var(--dim);
  background: var(--border);
  border-radius: 8px;
  padding: 1px 6px;
}

/* Empty state */
.sv-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 40px 20px;
  color: var(--dim);
  font-size: 12px;
}

/* Scrollable list */
.sv-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
  padding-right: 4px;
}
.sv-list::-webkit-scrollbar       { width: 4px; }
.sv-list::-webkit-scrollbar-track { background: transparent; }
.sv-list::-webkit-scrollbar-thumb { background: var(--border-hi); border-radius: 4px; }

/* ── Schedule card ───────────────────────────────────────────────────────────── */
.sched-card {
  display: flex;
  align-items: center;
  gap: 12px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 12px 14px;
  transition: border-color .15s, opacity .2s;
}
.sched-card:hover       { border-color: var(--border-hi); }
.sched-card.is-disabled { opacity: .5; }

.sched-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }

.sched-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sched-meta {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--muted);
}
.sched-sep { color: var(--dim); }
.sched-when {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--muted);
}

/* Recurrence badge */
.sched-rec {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: .04em;
  text-transform: uppercase;
  padding: 1px 6px;
  border-radius: 8px;
}
.rec-once   { background: var(--border);    color: var(--muted); }
.rec-daily  { background: var(--blue-bg);   color: var(--blue);  }
.rec-weekly { background: var(--purple-bg); color: var(--purple);}

/* Actions */
.sched-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}
.sched-icon-btn {
  width: 26px; height: 26px;
  border: 1px solid var(--border-hi);
  border-radius: var(--radius);
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  transition: background .12s, color .12s, border-color .12s;
}
.sched-icon-btn:hover         { background: var(--bg-hover); color: var(--text); }
.sched-icon-btn.danger:hover  { background: var(--red-bg); color: var(--red); border-color: var(--red); }

/* Toggle switch */
.sched-toggle {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 2px;
  display: flex;
  align-items: center;
}
.toggle-track {
  position: relative;
  display: block;
  width: 30px; height: 16px;
  border-radius: 10px;
  background: var(--border-hi);
  transition: background .2s;
}
.sched-toggle.on .toggle-track { background: var(--blue); }
.toggle-thumb {
  position: absolute;
  top: 2px; left: 2px;
  width: 12px; height: 12px;
  border-radius: 50%;
  background: var(--muted);
  transition: transform .2s, background .2s;
}
.sched-toggle.on .toggle-thumb {
  transform: translateX(14px);
  background: #fff;
}
</style>
