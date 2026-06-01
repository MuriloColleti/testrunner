<template>
  <div class="layout">

    <!-- Sidebar -->
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

      <div class="sidebar-nav">
        <button class="snav-btn" :class="{ active: currentView === 'runner' }" @click="currentView = 'runner'">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
            <polygon points="3,2 13,8 3,14" stroke-linejoin="round"/>
          </svg>
          Testes
        </button>
        <button class="snav-btn" :class="{ active: currentView === 'dashboard' }" @click="currentView = 'dashboard'">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
            <rect x="1" y="8" width="3" height="6"/><rect x="6" y="4" width="3" height="10"/><rect x="11" y="1" width="3" height="13"/>
          </svg>
          Dashboard
        </button>
        <button class="snav-btn" :class="{ active: currentView === 'scheduler' }" @click="currentView = 'scheduler'">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" width="13" height="13">
            <rect x="2" y="3" width="12" height="12" rx="1.5"/>
            <path d="M5 1v4M11 1v4M2 8h12"/>
          </svg>
          Agendamentos
          <span v-if="todayScheduleCount > 0" class="snav-badge">{{ todayScheduleCount }}</span>
        </button>
      </div>

      <div v-show="currentView === 'runner'" class="sidebar-body">
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
            <div v-for="group in suitesByTag(proj.suites)" :key="group.tag" class="tag-group">
              <button class="tag-group-hd" @click="toggleGroup(proj.id, group.tag)">
                <svg class="tg-chevron" :class="{ open: !isCollapsed(proj.id, group.tag) }" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.8" width="10" height="10">
                  <path d="M3 4.5l3 3 3-3" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                <span class="tag sm" :class="tagClass(group.tag)">{{ group.tag }}</span>
                <span class="tg-count">{{ group.suites.length }}</span>
              </button>
              <div v-show="!isCollapsed(proj.id, group.tag)" class="tag-group-body">
                <div
                  v-for="s in group.suites"
                  :key="s.id"
                  class="suite-row"
                  :class="{ active: activeTab === s.id }"
                  @click="activateTab(s.id)"
                >
                  <span class="suite-dot" :class="runStatus(s.id)"></span>
                  <div class="suite-info">
                    <span class="suite-label">{{ s.system }} &middot; {{ s.name }}</span>
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
            </div>
          </div>
        </template>
      </div>
    </aside>

    <!-- Main -->
    <main class="main">
      <TestRunner    v-if="currentView === 'runner'"    @add-project="openModal()" />
      <Dashboard     v-if="currentView === 'dashboard'" />
      <SchedulerView v-if="currentView === 'scheduler'" />
    </main>

    <!-- Config Modal -->
    <Transition name="modal">
      <div v-if="modal.open" class="modal-overlay" @click.self="closeModal">
        <div class="modal">
          <div class="modal-header">
            <span>{{ modal.title }}</span>
            <button class="modal-close" @click="closeModal">&times;</button>
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
import { reactive, ref, computed, onMounted, nextTick, toRaw } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useTestState }  from './composables/useTestState'
import { useScheduler }  from './composables/useScheduler'
import TestRunner        from './components/TestRunner.vue'
import Dashboard         from './components/Dashboard.vue'
import SchedulerView     from './components/SchedulerView.vue'

const {
  projects, activeTab, currentView,
  runStatus, tagClass, suitesByTag, isCollapsed, toggleGroup,
  handleRunBtn, activateTab, loadProjects, registerListeners,
} = useTestState()

const { schedules, loadSchedules, registerSchedulerListeners } = useScheduler()

// Badge: agendamentos habilitados para hoje
const todayScheduleCount = computed(() => {
  const today = new Date().toISOString().slice(0, 10)
  return schedules.value.filter(s => s.enabled && s.scheduledAt?.startsWith(today)).length
})

// ── Modal state ──────────────────────────────────────────────────────────────
const modal = reactive({
  open: false, title: '', editingId: null,
  name: '', path: '',
  pendingSuites: [], verifying: false,
  verifyOk: false, verifyMsg: '',
})
const pathInput = ref(null)

// ── Boot ─────────────────────────────────────────────────────────────────────
onMounted(async () => {
  await loadProjects()
  await registerListeners()
  await loadSchedules()
  await registerSchedulerListeners()
})

// ── Modal functions ──────────────────────────────────────────────────────────
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
      modal.verifyMsg = `\u2713 ${p.suites.length} suite(s) encontrada(s).`
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
    modal.verifyMsg = `\u2713 ${suites.length} suite(s) encontrada(s).`
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
