<template>
  <div class="cal-wrap">

    <!-- Header: mês + navegação -->
    <div class="cal-header">
      <button class="cal-nav" @click="prevMonth">
        <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.8" width="10" height="10">
          <path d="M6.5 2L3.5 5l3 3" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
      <span class="cal-title">{{ monthLabel }}</span>
      <button class="cal-nav" @click="nextMonth">
        <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.8" width="10" height="10">
          <path d="M3.5 2L6.5 5l-3 3" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
    </div>

    <!-- Dias da semana -->
    <div class="cal-dow">
      <span v-for="d in DOW" :key="d">{{ d }}</span>
    </div>

    <!-- Grid de dias -->
    <div class="cal-grid">
      <button
        v-for="cell in cells"
        :key="cell.key"
        class="cal-cell"
        :class="{
          'out':      !cell.inMonth,
          'today':     cell.isToday,
          'selected':  cell.dateStr === selectedDate,
          'has-sched': cell.count > 0,
        }"
        @click="cell.inMonth && $emit('day-click', cell.dateStr)"
      >
        <span class="cal-day">{{ cell.day }}</span>
        <span v-if="cell.count > 0 && cell.inMonth" class="cal-dot" :title="`${cell.count} agendamento(s)`"></span>
      </button>
    </div>

  </div>
</template>

<script setup>
import { ref, computed } from 'vue'

const props = defineProps({
  schedules:    { type: Array,  default: () => [] },
  selectedDate: { type: String, default: '' },
})

defineEmits(['day-click'])

// ── Estado interno do calendário ──────────────────────────────────────────────
const today     = new Date()
const viewYear  = ref(today.getFullYear())
const viewMonth = ref(today.getMonth()) // 0-indexed

const DOW = ['Dom', 'Seg', 'Ter', 'Qua', 'Qui', 'Sex', 'Sáb']

const MONTHS_PT = [
  'Janeiro','Fevereiro','Março','Abril','Maio','Junho',
  'Julho','Agosto','Setembro','Outubro','Novembro','Dezembro',
]

// ── Computed ──────────────────────────────────────────────────────────────────
const monthLabel = computed(() =>
  `${MONTHS_PT[viewMonth.value]} ${viewYear.value}`
)

const todayStr = computed(() => fmtDate(today))

/** Mapa: "YYYY-MM-DD" → quantidade de schedules nesse dia */
const scheduleMap = computed(() => {
  const map = {}
  for (const s of props.schedules) {
    if (!s.scheduledAt) continue
    const day = s.scheduledAt.slice(0, 10)
    map[day] = (map[day] ?? 0) + 1
  }
  return map
})

/** Células do grid (6 semanas × 7 dias) */
const cells = computed(() => {
  const y = viewYear.value
  const m = viewMonth.value
  const firstDay = new Date(y, m, 1).getDay()   // 0=Dom
  const daysInMonth = new Date(y, m + 1, 0).getDate()

  const result = []
  let date = new Date(y, m, 1 - firstDay)

  for (let i = 0; i < 42; i++) {
    const ds  = fmtDate(date)
    const inM = date.getMonth() === m && date.getFullYear() === y
    result.push({
      key:      ds,
      day:      date.getDate(),
      dateStr:  ds,
      inMonth:  inM,
      isToday:  ds === todayStr.value,
      count:    inM ? (scheduleMap.value[ds] ?? 0) : 0,
    })
    date = new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1)
  }
  return result
})

// ── Navegação ─────────────────────────────────────────────────────────────────
function prevMonth() {
  if (viewMonth.value === 0) { viewMonth.value = 11; viewYear.value-- }
  else viewMonth.value--
}

function nextMonth() {
  if (viewMonth.value === 11) { viewMonth.value = 0; viewYear.value++ }
  else viewMonth.value++
}

// ── Util ──────────────────────────────────────────────────────────────────────
function fmtDate(d) {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}
</script>

<style scoped>
.cal-wrap {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
  user-select: none;
}

/* Header */
.cal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
}
.cal-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text);
  letter-spacing: .02em;
}
.cal-nav {
  width: 24px; height: 24px;
  border: none; border-radius: var(--radius);
  background: transparent; color: var(--muted);
  cursor: pointer; display: flex; align-items: center; justify-content: center;
  transition: background .12s, color .12s;
}
.cal-nav:hover { background: var(--border-hi); color: var(--text); }

/* Days of week */
.cal-dow {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  padding: 8px 8px 4px;
  gap: 2px;
}
.cal-dow span {
  text-align: center;
  font-size: 10px;
  font-weight: 600;
  color: var(--dim);
  text-transform: uppercase;
  letter-spacing: .05em;
}

/* Grid */
.cal-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  padding: 4px 8px 8px;
  gap: 2px;
}

.cal-cell {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  height: 36px;
  border: none;
  border-radius: var(--radius);
  background: transparent;
  cursor: pointer;
  transition: background .1s;
}
.cal-cell.out       { opacity: .2; pointer-events: none; cursor: default; }
.cal-cell:not(.out):hover { background: var(--bg-hover); }
.cal-cell.today     .cal-day { color: var(--blue); font-weight: 700; }
.cal-cell.selected  { background: var(--blue-bg) !important; }
.cal-cell.selected  .cal-day { color: var(--blue); }

.cal-day {
  font-size: 11px;
  color: var(--muted);
  line-height: 1;
}
.cal-cell.today .cal-day,
.cal-cell.has-sched .cal-day { color: var(--text); }

/* Dot indicator */
.cal-dot {
  width: 4px; height: 4px;
  border-radius: 50%;
  background: var(--blue);
  flex-shrink: 0;
}
.cal-cell.selected .cal-dot { background: var(--blue); }
</style>
