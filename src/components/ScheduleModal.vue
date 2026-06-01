<template>
  <div class="modal-overlay" @click.self="$emit('close')">
    <div class="modal sched-modal">

      <div class="modal-header">
        <span>{{ editSchedule ? 'Editar Agendamento' : 'Novo Agendamento' }}</span>
        <button class="modal-close" @click="$emit('close')">&times;</button>
      </div>

      <div class="modal-body">

        <!-- Label -->
        <div class="form-group">
          <label>Nome do Agendamento</label>
          <input
            v-model="form.label"
            type="text"
            placeholder='Ex: "Nightly E2E Frontend"'
            autocomplete="off"
          />
        </div>

        <!-- Projeto + Suite -->
        <div class="form-row">
          <div class="form-group">
            <label>Projeto</label>
            <select v-model="form.projectId" @change="form.suiteId = ''">
              <option value="" disabled>Selecionar...</option>
              <option v-for="p in projects" :key="p.id" :value="p.id">{{ p.name }}</option>
            </select>
          </div>
          <div class="form-group">
            <label>Suite</label>
            <select v-model="form.suiteId" :disabled="!form.projectId">
              <option value="" disabled>Selecionar...</option>
              <option
                v-for="s in selectedSuites"
                :key="s.id"
                :value="s.id"
              >{{ s.system }} · {{ s.name }}</option>
            </select>
          </div>
        </div>

        <!-- Recorrência -->
        <div class="form-group">
          <label>Recorrência</label>
          <div class="recurrence-opts">
            <label
              v-for="opt in recurrenceOpts"
              :key="opt.value"
              class="rec-opt"
              :class="{ active: form.recurrence === opt.value }"
            >
              <input type="radio" v-model="form.recurrence" :value="opt.value" />
              <span>{{ opt.label }}</span>
            </label>
          </div>
        </div>

        <!-- Dia da semana — somente semanal -->
        <Transition name="fade-down">
          <div v-if="form.recurrence === 'weekly'" class="form-group">
            <label>Dia da Semana</label>
            <div class="weekday-opts">
              <button
                v-for="(d, i) in WEEKDAYS"
                :key="i"
                type="button"
                class="weekday-btn"
                :class="{ active: form.weekday === i }"
                @click="form.weekday = i"
              >{{ d }}</button>
            </div>
          </div>
        </Transition>

        <!-- Data + Hora (only once), ou só Hora (daily/weekly) -->
        <div :class="form.recurrence === 'once' ? 'form-row' : 'form-half-row'">
          <div v-if="form.recurrence === 'once'" class="form-group">
            <label>Data</label>
            <input v-model="form.date" type="date" />
          </div>
          <div class="form-group">
            <label>Horário</label>
            <input v-model="form.time" type="time" />
          </div>
        </div>

      </div><!-- /modal-body -->

      <div class="modal-footer">
        <button v-if="editSchedule" class="btn-danger" @click="handleDelete">Remover</button>
        <div class="footer-right">
          <button class="btn-secondary" @click="$emit('close')">Cancelar</button>
          <button class="btn-primary" @click="handleSave" :disabled="!isValid">Salvar</button>
        </div>
      </div>

    </div>
  </div>
</template>

<script setup>
import { reactive, computed, watch } from 'vue'
import { useTestState }  from '../composables/useTestState'
import { useScheduler }  from '../composables/useScheduler'

// ── Props / Emits ─────────────────────────────────────────────────────────────
const props = defineProps({
  editSchedule: { type: Object, default: null },
})
const emit = defineEmits(['close'])

// ── External state ────────────────────────────────────────────────────────────
const { projects }                      = useTestState()
const { saveSchedule, deleteSchedule }  = useScheduler()

// ── Constantes ────────────────────────────────────────────────────────────────
const WEEKDAYS = ['Dom', 'Seg', 'Ter', 'Qua', 'Qui', 'Sex', 'Sáb']

const recurrenceOpts = [
  { value: 'once',   label: 'Uma vez' },
  { value: 'daily',  label: 'Diário'  },
  { value: 'weekly', label: 'Semanal' },
]

// ── Form state ────────────────────────────────────────────────────────────────
const form = reactive({
  label:      '',
  projectId:  '',
  suiteId:    '',
  date:       todayStr(),
  time:       '08:00',
  recurrence: 'once',
  weekday:    null,   // 0-6, apenas para 'weekly'
})

// ── Computed ──────────────────────────────────────────────────────────────────
const selectedSuites = computed(() => {
  const proj = projects.value.find(p => p.id === form.projectId)
  return proj?.suites ?? []
})

const isValid = computed(() => {
  if (!form.label.trim() || !form.projectId || !form.suiteId || !form.time) return false
  if (form.recurrence === 'once'   && !form.date)                return false
  if (form.recurrence === 'weekly' && form.weekday === null)     return false
  return true
})

// ── Pre-fill ao editar ────────────────────────────────────────────────────────
watch(() => props.editSchedule, (s) => {
  if (!s) return
  const [datePart, timePart] = s.scheduledAt.split('T')
  form.label      = s.label
  form.projectId  = s.projectId
  form.suiteId    = s.suiteId
  form.recurrence = s.recurrence
  form.time       = (timePart ?? '08:00').slice(0, 5)

  if (s.recurrence === 'once') {
    form.date    = datePart ?? todayStr()
    form.weekday = null
  } else if (s.recurrence === 'daily') {
    form.weekday = null
  } else {
    // weekly: extrai dia da semana da data armazenada
    form.weekday = new Date(s.scheduledAt).getDay()
  }
}, { immediate: true })

// ── Salvar ────────────────────────────────────────────────────────────────────
async function handleSave() {
  if (!isValid.value) return
  const schedule = {
    id:          props.editSchedule?.id ?? crypto.randomUUID(),
    projectId:   form.projectId,
    suiteId:     form.suiteId,
    label:       form.label.trim(),
    scheduledAt: computeScheduledAt(),
    recurrence:  form.recurrence,
    enabled:     true,
    lastRunAt:   props.editSchedule?.lastRunAt ?? null,
    createdAt:   props.editSchedule?.createdAt ?? '',
  }
  try {
    await saveSchedule(schedule)
    emit('close')
  } catch (e) {
    console.error('[ScheduleModal] erro ao salvar:', e)
  }
}

async function handleDelete() {
  if (!props.editSchedule || !confirm('Remover este agendamento?')) return
  try {
    await deleteSchedule(props.editSchedule.id)
    emit('close')
  } catch (e) {
    console.error('[ScheduleModal] erro ao deletar:', e)
  }
}

// ── Cálculo da próxima ocorrência ─────────────────────────────────────────────
function computeScheduledAt() {
  const [h, m] = form.time.split(':').map(Number)
  const now = new Date()

  if (form.recurrence === 'once') {
    return `${form.date}T${form.time}:00`
  }

  if (form.recurrence === 'daily') {
    // próxima ocorrência: hoje se ainda não passou, senão amanhã
    const candidate = new Date(now.getFullYear(), now.getMonth(), now.getDate(), h, m, 0)
    if (candidate <= now) candidate.setDate(candidate.getDate() + 1)
    return fmtLocalDatetime(candidate)
  }

  // weekly: próxima ocorrência do dia da semana escolhido
  const targetDow = form.weekday
  const currentDow = now.getDay()
  let daysUntil = (targetDow - currentDow + 7) % 7
  const candidate = new Date(now.getFullYear(), now.getMonth(), now.getDate() + daysUntil, h, m, 0)
  if (candidate <= now) candidate.setDate(candidate.getDate() + 7)
  return fmtLocalDatetime(candidate)
}

// ── Utils ─────────────────────────────────────────────────────────────────────
function todayStr() {
  return new Date().toISOString().slice(0, 10)
}

function fmtLocalDatetime(d) {
  const pad = n => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}:00`
}
</script>

<style scoped>
.sched-modal { width: 460px; }

/* Rows */
.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

/* Horário sozinho — metade da largura */
.form-half-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
.form-half-row .form-group:only-child {
  grid-column: 1 / 2;
}

/* Select */
select {
  width: 100%;
  background: var(--bg);
  border: 1px solid var(--border-hi);
  border-radius: var(--radius);
  color: var(--text);
  font-family: var(--font-ui);
  font-size: 12px;
  padding: 7px 28px 7px 10px;
  outline: none;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'%3E%3Cpath d='M0 0l5 6 5-6z' fill='%238b949e'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 10px center;
  transition: border-color .15s;
  cursor: pointer;
}
select:focus    { border-color: var(--blue); }
select:disabled { opacity: .4; cursor: not-allowed; }
select option   { background: var(--bg-card); }

input[type="date"],
input[type="time"] { color-scheme: dark; }

/* Recorrência */
.recurrence-opts {
  display: flex;
  gap: 8px;
}
.rec-opt {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 7px 10px;
  border: 1px solid var(--border-hi);
  border-radius: var(--radius);
  cursor: pointer;
  font-size: 12px;
  color: var(--muted);
  transition: border-color .15s, background .15s, color .15s;
  user-select: none;
}
.rec-opt:hover         { border-color: var(--blue); color: var(--text); }
.rec-opt.active        { border-color: var(--blue); background: var(--blue-bg); color: var(--blue); }
.rec-opt input[type="radio"] { display: none; }

/* Dias da semana */
.weekday-opts {
  display: flex;
  gap: 6px;
}
.weekday-btn {
  flex: 1;
  padding: 7px 4px;
  border: 1px solid var(--border-hi);
  border-radius: var(--radius);
  background: transparent;
  color: var(--muted);
  font-family: var(--font-ui);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: border-color .15s, background .15s, color .15s;
}
.weekday-btn:hover  { border-color: var(--blue); color: var(--text); }
.weekday-btn.active { border-color: var(--blue); background: var(--blue-bg); color: var(--blue); font-weight: 700; }

/* Animação de entrada dos dias da semana */
.fade-down-enter-active { transition: opacity .18s ease, transform .18s ease; }
.fade-down-leave-active { transition: opacity .12s ease, transform .12s ease; }
.fade-down-enter-from   { opacity: 0; transform: translateY(-6px); }
.fade-down-leave-to     { opacity: 0; transform: translateY(-4px); }
</style>
