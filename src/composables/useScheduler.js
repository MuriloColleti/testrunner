import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTestState } from './useTestState'

// ── State (module-level singletons) ──────────────────────────────────────────
const schedules = ref([])
let listenersRegistered = false

// ── CRUD ──────────────────────────────────────────────────────────────────────
async function loadSchedules() {
  try { schedules.value = await invoke('get_schedules') }
  catch (e) { console.error('[loadSchedules] erro:', e) }
}

async function saveSchedule(schedule) {
  await invoke('save_schedule', { schedule })
  await loadSchedules()
}

async function deleteSchedule(scheduleId) {
  await invoke('delete_schedule', { scheduleId })
  await loadSchedules()
}

async function toggleSchedule(scheduleId, enabled) {
  await invoke('toggle_schedule', { scheduleId, enabled })
  // atualiza localmente para evitar flicker
  const s = schedules.value.find(x => x.id === scheduleId)
  if (s) s.enabled = enabled
}

// ── Listeners ─────────────────────────────────────────────────────────────────
async function registerSchedulerListeners() {
  if (listenersRegistered) return
  listenersRegistered = true

  const { startRun } = useTestState()

  await listen('schedule-triggered', ({ payload }) => {
    console.log('[scheduler] disparando agendamento:', payload.label)
    startRun(payload.suiteId, payload.projectId)
  })

  await listen('schedules-updated', () => {
    loadSchedules()
  })
}

// ── Export ────────────────────────────────────────────────────────────────────
export function useScheduler() {
  return {
    schedules,
    loadSchedules,
    saveSchedule,
    deleteSchedule,
    toggleSchedule,
    registerSchedulerListeners,
  }
}
