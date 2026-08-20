<script setup lang="ts">
import { ref, onMounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import { useI18n } from '@/i18n'
import { useModulesStore } from '@/stores/modules'
import type { ModuleDto } from '@/api/modules'

const { t } = useI18n()
const modulesStore = useModulesStore()
const viewMode = ref<'table' | 'graph'>('table')
const isScanning = ref(false)
const showInstallModal = ref(false)
const selectedFile = ref<File | null>(null)
const isInstalling = ref(false)

onMounted(() => {
  modulesStore.fetchModules()
})

async function handleScan() {
  isScanning.value = true
  try {
    await modulesStore.fetchModules()
  } finally {
    setTimeout(() => {
      isScanning.value = false
    }, 600)
  }
}

async function handleToggle(mod: ModuleDto) {
  try {
    await modulesStore.toggleModule(mod.id, !mod.is_active)
  } catch (err) {
    console.error('Failed to toggle module:', err)
  }
}

function handleFileChange(e: Event) {
  const target = e.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    selectedFile.value = target.files[0]
  }
}

function handleDrop(e: DragEvent) {
  e.preventDefault()
  if (e.dataTransfer && e.dataTransfer.files.length > 0) {
    selectedFile.value = e.dataTransfer.files[0]
  }
}

async function handleInstall() {
  if (!selectedFile.value) return
  isInstalling.value = true
  try {
    // Имитация загрузки и установки WASM модуля
    await new Promise((resolve) => setTimeout(resolve, 1000))
    await modulesStore.fetchModules()
    showInstallModal.value = false
    selectedFile.value = null
  } finally {
    isInstalling.value = false
  }
}

function formatPermission(p: any): string {
  if (typeof p === 'string') return p
  if (p && typeof p === 'object') return p.id || p.name || 'permission'
  return String(p)
}

function getPermissionTitle(p: any): string {
  if (p && typeof p === 'object') return p.description || p.name || p.id || ''
  return String(p)
}
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)] select-none">
    <!-- Top Settings Subnavigation -->
    <SettingsNav />

    <!-- Main Content Area with Aside -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="relative z-10 p-lg flex gap-lg h-full w-full">
        <!-- Submenu Sidebar -->
        <aside class="w-nav-width shrink-0 hidden md:flex flex-col gap-sm border-r border-outline-variant pr-md">
          <div class="px-md mb-sm">
            <div class="flex flex-col gap-xs">
              <router-link
                to="/settings/modules"
                class="flex items-center gap-md px-md py-sm rounded-lg bg-gradient-to-r from-primary-fixed-dim/20 to-transparent border-l-2 border-primary-fixed-dim text-primary-fixed-dim font-bold transition-all duration-200 shadow-[inset_0_0_10px_rgba(115,212,232,0.15)] hover:from-primary-fixed-dim/30 hover:to-transparent"
              >
                <span class="material-symbols-outlined" style="font-variation-settings: 'FILL' 1;">view_module</span>
                <h3 class="text-label-caps font-label-caps text-primary-fixed-dim uppercase tracking-wider">
                  {{ t('modules.title') }}
                </h3>
              </router-link>
            </div>
          </div>

          <div class="px-md mt-md">
            <h3 class="text-label-caps font-label-caps text-on-surface-variant uppercase tracking-wider mb-sm">
              {{ t('dashboard.nmsModules') }}
            </h3>
            <div class="flex flex-col gap-xs">
              <router-link
                to="/data-processor"
                class="flex items-center gap-md px-md py-sm rounded-lg text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all"
              >
                <span class="material-symbols-outlined text-[18px]">analytics</span>
                <span class="text-xs">{{ t('nav.dataProcessor') }}</span>
              </router-link>
              <router-link
                to="/file-explorer"
                class="flex items-center gap-md px-md py-sm rounded-lg text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all"
              >
                <span class="material-symbols-outlined text-[18px]">folder</span>
                <span class="text-xs">{{ t('nav.fileExplorer') }}</span>
              </router-link>
            </div>
          </div>
        </aside>

        <!-- Main Content -->
        <div class="flex-1 flex flex-col gap-lg w-full pb-xl">
          <!-- Header Actions -->
          <div class="flex items-center justify-between flex-wrap gap-md">
            <div>
              <h1 class="font-display-lg text-display-lg text-on-surface font-bold">
                {{ t('modules.title') }}
              </h1>
              <p class="text-sm text-on-surface-variant mt-1">
                {{ t('modules.subtitle') }}
              </p>
            </div>
            <div class="flex items-center gap-3">
              <button
                type="button"
                class="bg-surface-container-high hover:bg-surface-variant text-on-surface border border-outline-variant hover:border-primary-fixed-dim/40 px-4 py-2 rounded-lg text-xs font-bold uppercase flex items-center gap-2 active:scale-95 transition-all duration-200 hover:brightness-110 ease-in-out cursor-pointer"
                @click="handleScan"
              >
                <span class="material-symbols-outlined text-[18px]" :class="{ 'animate-spin': isScanning }">refresh</span>
                {{ t('modules.scanNewModules') }}
              </button>
              <button
                type="button"
                class="bg-primary-fixed-dim hover:bg-primary-fixed-dim/90 text-on-primary-fixed border border-primary-fixed-dim px-4 py-2 rounded-lg text-xs font-bold uppercase flex items-center gap-2 active:scale-95 transition-all duration-200 shadow-glow-primary-sm hover:shadow-glow-primary-md cursor-pointer"
                @click="showInstallModal = true"
              >
                <span class="material-symbols-outlined text-[18px]">add</span>
                {{ t('modules.installModule') }}
              </button>
            </div>
          </div>

          <!-- Metric Cards -->
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-md">
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark">
              <p class="text-label-caps font-label-caps text-on-surface-variant uppercase tracking-widest mb-sm">
                {{ t('modules.totalModules') }}
              </p>
              <p class="text-[32px] leading-none font-body-mono font-bold text-on-surface">
                {{ modulesStore.totalCount }}
              </p>
            </div>
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark">
              <p class="text-label-caps font-label-caps text-tertiary-fixed-dim uppercase tracking-widest mb-sm">
                {{ t('modules.active') }}
              </p>
              <p class="text-[32px] leading-none font-body-mono font-bold text-tertiary-fixed-dim">
                {{ modulesStore.activeCount }}
              </p>
            </div>
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark">
              <p class="text-label-caps font-label-caps text-on-surface-variant uppercase tracking-widest mb-sm">
                {{ t('modules.disabled') }}
              </p>
              <p class="text-[32px] leading-none font-body-mono font-bold text-on-surface">
                {{ modulesStore.disabledCount }}
              </p>
            </div>
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark">
              <p class="text-label-caps font-label-caps text-primary-fixed-dim uppercase tracking-widest mb-sm">
                {{ t('modules.moduleType') }}
              </p>
              <p class="text-[32px] leading-none font-body-mono font-bold text-primary-fixed-dim">
                {{ modulesStore.modules.length > 0 ? 'WASM' : '0' }}
              </p>
            </div>
          </div>

          <!-- Main Layout: Registry + Side Details -->
          <div class="flex flex-col lg:flex-row gap-lg">
            <!-- Left: Module Registry Card -->
            <div class="flex-1 bg-surface-container-low border border-outline-variant rounded-lg shadow-card-dark flex flex-col overflow-hidden">
              <!-- Card Header: Views & Filters -->
              <div class="p-md border-b border-outline-variant flex items-center justify-between flex-wrap gap-md bg-surface-container">
                <div class="flex items-center gap-lg">
                  <h2 class="font-title-sm text-title-sm text-on-surface font-bold">
                    {{ t('modules.moduleRegistry') }}
                  </h2>
                  <div class="flex bg-surface-container-lowest rounded-lg p-1 border border-outline-variant">
                    <button
                      type="button"
                      class="px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-2 transition-all cursor-pointer"
                      :class="viewMode === 'table'
                        ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm'
                        : 'text-on-surface-variant hover:text-on-surface'"
                      @click="viewMode = 'table'"
                    >
                      <span class="material-symbols-outlined text-[16px]">table_rows</span>
                      {{ t('modules.tableView') }}
                    </button>
                    <button
                      type="button"
                      class="px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-2 transition-all cursor-pointer"
                      :class="viewMode === 'graph'
                        ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm'
                        : 'text-on-surface-variant hover:text-on-surface'"
                      @click="viewMode = 'graph'"
                    >
                      <span class="material-symbols-outlined text-[16px]">account_tree</span>
                      {{ t('modules.topologyGraph') }}
                    </button>
                  </div>
                </div>

                <div class="flex items-center gap-md">
                  <span class="text-xs font-bold text-on-surface-variant uppercase tracking-wider">
                    {{ t('modules.filter') }}
                  </span>
                  <div class="flex bg-surface-container-lowest rounded-lg p-1 border border-outline-variant">
                    <button
                      type="button"
                      class="px-3 py-1 rounded-md text-xs font-bold uppercase tracking-wider transition-all cursor-pointer"
                      :class="modulesStore.filter === 'all'
                        ? 'bg-primary-fixed-dim text-on-primary-fixed'
                        : 'text-on-surface-variant hover:text-on-surface'"
                      @click="modulesStore.setFilter('all')"
                    >
                      {{ t('modules.all') }}
                    </button>
                    <button
                      type="button"
                      class="px-3 py-1 rounded-md text-xs font-bold uppercase tracking-wider transition-all cursor-pointer"
                      :class="modulesStore.filter === 'active'
                        ? 'bg-primary-fixed-dim text-on-primary-fixed'
                        : 'text-on-surface-variant hover:text-on-surface'"
                      @click="modulesStore.setFilter('active')"
                    >
                      {{ t('modules.activeStatus') }}
                    </button>
                    <button
                      type="button"
                      class="px-3 py-1 rounded-md text-xs font-bold uppercase tracking-wider transition-all cursor-pointer"
                      :class="modulesStore.filter === 'disabled'
                        ? 'bg-primary-fixed-dim text-on-primary-fixed'
                        : 'text-on-surface-variant hover:text-on-surface'"
                      @click="modulesStore.setFilter('disabled')"
                    >
                      {{ t('modules.disabledStatus') }}
                    </button>
                  </div>
                </div>
              </div>

              <!-- Table View -->
              <div v-if="viewMode === 'table'" class="overflow-x-auto">
                <table class="w-full text-left border-collapse">
                  <thead class="bg-surface-variant/50 border-b border-outline-variant">
                    <tr>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest w-1/4">
                        {{ t('modules.moduleName') }}
                      </th>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest w-1/5">
                        {{ t('modules.moduleTypeCol') }}
                      </th>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest w-1/6">
                        {{ t('modules.version') }}
                      </th>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest w-1/6">
                        {{ t('modules.status') }}
                      </th>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest text-right">
                        {{ t('modules.actions') }}
                      </th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-outline-variant/30 text-xs">
                    <tr
                      v-for="mod in modulesStore.filteredModules"
                      :key="mod.id"
                      class="hover:bg-surface-variant/20 transition-colors cursor-pointer"
                      :class="{ 'bg-surface-variant/40': modulesStore.selectedModule?.id === mod.id }"
                      @click="modulesStore.selectModule(mod)"
                    >
                      <td class="py-md px-md">
                        <div class="flex items-center gap-3">
                          <div class="w-8 h-8 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                            <span class="material-symbols-outlined text-[18px]">extension</span>
                          </div>
                          <div>
                            <p class="font-bold text-on-surface text-sm">{{ mod.name }}</p>
                            <p class="text-[11px] font-body-mono text-on-surface-variant">{{ mod.id }}</p>
                          </div>
                        </div>
                      </td>
                      <td class="py-md px-md font-body-mono text-xs text-on-surface-variant">
                        {{ mod.manifest?.ui?.type || 'WASM Core' }}
                      </td>
                      <td class="py-md px-md font-body-mono text-xs text-on-surface-variant">
                        v{{ mod.version }}
                      </td>
                      <td class="py-md px-md">
                        <span
                          class="px-2 py-0.5 rounded text-[10px] font-bold uppercase font-body-mono"
                          :class="mod.is_active
                            ? 'bg-tertiary-fixed-dim/15 text-tertiary-fixed-dim border border-tertiary-fixed-dim/30'
                            : 'bg-surface-variant text-on-surface-variant border border-outline-variant/40'"
                        >
                          {{ mod.is_active ? t('modules.activeStatus') : t('modules.disabledStatus') }}
                        </span>
                      </td>
                      <td class="py-md px-md text-right">
                        <button
                          type="button"
                          class="px-3 py-1 text-xs font-semibold rounded-lg border transition-all cursor-pointer active:scale-95"
                          :class="mod.is_active
                            ? 'border-error text-error hover:bg-error-container/20'
                            : 'border-tertiary-fixed-dim text-tertiary-fixed-dim hover:bg-tertiary-fixed-dim/15'"
                          @click.stop="handleToggle(mod)"
                        >
                          {{ mod.is_active ? t('modules.disable') : t('modules.enable') }}
                        </button>
                      </td>
                    </tr>

                    <tr v-if="modulesStore.filteredModules.length === 0">
                      <td class="py-xl px-md text-center text-sm text-on-surface-variant" colspan="5">
                        {{ t('modules.noModulesFound') }}
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>

              <!-- Interactive Topology Graph View -->
              <div v-else class="p-lg flex flex-col items-center justify-center min-h-[380px] text-center bg-surface-container-lowest/50 relative overflow-hidden">
                <!-- Visual Mesh Network Canvas -->
                <div class="w-full max-w-2xl py-6 flex flex-col items-center gap-8 relative z-10">
                  <!-- Central Hub: NMS Core Bus -->
                  <div class="px-6 py-3 rounded-xl bg-surface-container-highest border-2 border-primary-fixed-dim text-primary-fixed-dim shadow-glow-primary-md flex items-center gap-3">
                    <span class="material-symbols-outlined text-2xl animate-pulse">hub</span>
                    <div class="text-left">
                      <div class="text-xs font-bold font-body-mono uppercase">AetherCore Message Bus</div>
                      <div class="text-[10px] text-on-surface-variant">IPC / WASM Host Runtime</div>
                    </div>
                  </div>

                  <!-- Connected Module Nodes -->
                  <div class="grid grid-cols-2 sm:grid-cols-3 gap-6 w-full">
                    <div
                      v-for="mod in modulesStore.modules"
                      :key="mod.id"
                      class="p-3 rounded-lg border flex flex-col items-center gap-2 cursor-pointer transition-all hover:scale-105"
                      :class="mod.is_active
                        ? 'bg-surface-container border-tertiary-fixed-dim/40 shadow-glow-tertiary-sm'
                        : 'bg-surface-container/60 border-outline-variant opacity-60'"
                      @click="modulesStore.selectModule(mod)"
                    >
                      <div class="w-10 h-10 rounded-full flex items-center justify-center" :class="mod.is_active ? 'bg-tertiary-fixed-dim/20 text-tertiary-fixed-dim' : 'bg-surface-variant text-on-surface-variant'">
                        <span class="material-symbols-outlined text-xl">extension</span>
                      </div>
                      <span class="text-xs font-bold text-on-surface">{{ mod.name }}</span>
                      <span class="text-[10px] font-body-mono text-on-surface-variant">v{{ mod.version }}</span>
                    </div>

                    <!-- Host System Adapter Node -->
                    <div class="p-3 rounded-lg border border-outline-variant bg-surface-container flex flex-col items-center gap-2">
                      <div class="w-10 h-10 rounded-full bg-surface-variant flex items-center justify-center text-primary-fixed-dim">
                        <span class="material-symbols-outlined text-xl">router</span>
                      </div>
                      <span class="text-xs font-bold text-on-surface">Network Driver</span>
                      <span class="text-[10px] font-body-mono text-on-surface-variant">Sys Socket</span>
                    </div>
                  </div>
                </div>

                <div class="mt-4 text-center">
                  <h3 class="text-on-surface font-title-sm text-sm font-bold">{{ t('modules.topologyTitle') }}</h3>
                  <p class="text-xs text-on-surface-variant max-w-sm mt-1">
                    {{ t('modules.topologyDesc') }}
                  </p>
                </div>
              </div>
            </div>

            <!-- Right: Module Details Card -->
            <div class="w-full lg:w-80 shrink-0">
              <div
                v-if="modulesStore.selectedModule"
                class="bg-surface-container-low border border-outline-variant rounded-lg p-lg shadow-card-dark flex flex-col gap-md"
              >
                <div class="flex items-center justify-between border-b border-outline-variant/40 pb-sm">
                  <div>
                    <h3 class="font-title-sm text-sm text-on-surface font-bold">
                      {{ modulesStore.selectedModule.name }}
                    </h3>
                    <p class="text-[11px] font-body-mono text-on-surface-variant">
                      {{ modulesStore.selectedModule.id }}
                    </p>
                  </div>
                  <span class="text-[10px] font-body-mono text-primary-fixed-dim font-bold px-2 py-0.5 rounded bg-primary-fixed-dim/10 border border-primary-fixed-dim/30">
                    v{{ modulesStore.selectedModule.version }}
                  </span>
                </div>

                <div>
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">
                    {{ t('modules.description') }}
                  </label>
                  <p class="text-xs text-on-surface leading-relaxed">
                    {{ modulesStore.selectedModule.manifest?.description || t('modules.noDescription') }}
                  </p>
                </div>

                <div>
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">
                    {{ t('modules.author') }}
                  </label>
                  <p class="text-xs text-on-surface font-body-mono">
                    {{ modulesStore.selectedModule.manifest?.author || 'AetherCore Team' }}
                  </p>
                </div>

                <div>
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">
                    {{ t('modules.permissions') }}
                  </label>
                  <div class="flex flex-wrap gap-1.5">
                    <span
                      v-for="p in modulesStore.selectedModule.manifest?.permissions || ['network.listen', 'storage.kv', 'events.publish']"
                      :key="formatPermission(p)"
                      :title="getPermissionTitle(p)"
                      class="px-2 py-0.5 bg-surface-variant border border-outline-variant/60 text-[10px] font-body-mono text-primary-fixed-dim rounded"
                    >
                      {{ formatPermission(p) }}
                    </span>
                  </div>
                </div>

                <div class="mt-md pt-sm border-t border-outline-variant/40 flex justify-end">
                  <button
                    type="button"
                    class="w-full py-2 rounded-lg text-xs font-bold uppercase tracking-wider transition-all cursor-pointer active:scale-95"
                    :class="modulesStore.selectedModule.is_active
                      ? 'bg-error-container/20 text-error hover:bg-error-container/40 border border-error/40'
                      : 'bg-primary-fixed-dim text-on-primary-fixed hover:bg-primary-fixed-dim/90 shadow-glow-primary-sm'"
                    @click="handleToggle(modulesStore.selectedModule)"
                  >
                    {{ modulesStore.selectedModule.is_active ? t('modules.disable') : t('modules.enable') }}
                  </button>
                </div>
              </div>

              <div
                v-else
                class="bg-surface-container-low border border-outline-variant rounded-lg p-xl flex items-center justify-center text-center shadow-card-dark h-32"
              >
                <p class="text-sm text-on-surface-variant">
                  {{ t('modules.selectModulePrompt') }}
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- Modal: Install Module -->
    <div
      v-if="showInstallModal"
      class="fixed inset-0 bg-black/70 backdrop-blur-xs flex items-center justify-center z-50 p-md animate-fade-in"
      @click.self="showInstallModal = false"
    >
      <div class="bg-surface-container-low border border-outline-variant rounded-xl p-lg max-w-lg w-full shadow-2xl flex flex-col gap-md">
        <div class="flex items-center justify-between border-b border-outline-variant/60 pb-sm">
          <div class="flex items-center gap-2 text-primary-fixed-dim">
            <span class="material-symbols-outlined text-xl">add_box</span>
            <h3 class="text-sm font-bold text-on-surface">{{ t('modules.uploadWasmTitle') }}</h3>
          </div>
          <button
            type="button"
            class="text-on-surface-variant hover:text-on-surface transition-colors cursor-pointer"
            @click="showInstallModal = false"
          >
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </div>

        <p class="text-xs text-on-surface-variant">
          {{ t('modules.uploadWasmDesc') }}
        </p>

        <!-- Drop Zone -->
        <div
          class="border-2 border-dashed border-outline-variant hover:border-primary-fixed-dim/60 rounded-lg p-xl flex flex-col items-center justify-center text-center cursor-pointer transition-colors bg-surface-container-lowest/50"
          @dragover.prevent
          @drop="handleDrop"
          @click="($refs.fileInput as HTMLInputElement)?.click()"
        >
          <input
            ref="fileInput"
            type="file"
            accept=".wasm,.tar.gz,.zip"
            class="hidden"
            @change="handleFileChange"
          />
          <span class="material-symbols-outlined text-3xl text-primary-fixed-dim mb-2">cloud_upload</span>
          <p class="text-xs text-on-surface font-semibold mb-1">
            {{ selectedFile ? selectedFile.name : t('modules.dragDropFile') }}
          </p>
          <p class="text-[10px] text-on-surface-variant font-body-mono">
            {{ selectedFile ? `${(selectedFile.size / 1024).toFixed(1)} KB` : t('modules.browseFiles') }}
          </p>
        </div>

        <div class="flex items-center justify-end gap-2 pt-sm border-t border-outline-variant/60">
          <button
            type="button"
            class="px-4 py-1.5 text-xs font-semibold rounded-lg border border-outline-variant text-on-surface-variant hover:bg-surface-variant transition-colors cursor-pointer"
            @click="showInstallModal = false"
          >
            {{ t('modules.cancel') }}
          </button>
          <button
            type="button"
            class="px-4 py-1.5 text-xs font-bold rounded-lg bg-primary-fixed-dim text-on-primary-fixed hover:bg-primary-fixed-dim/90 shadow-glow-primary-sm transition-all cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed flex items-center gap-2"
            :disabled="!selectedFile || isInstalling"
            @click="handleInstall"
          >
            <span v-if="isInstalling" class="material-symbols-outlined text-sm animate-spin">refresh</span>
            {{ isInstalling ? t('modules.installing') : t('modules.install') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
