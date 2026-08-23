<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import {
  PageHeader,
  AppButton,
  BaseCard,
  StatusBadge,
  BaseModal,
  BaseSelect
} from '@/components/common'
import { useI18n } from '@/i18n'
import { useAuthStore } from '@/stores/auth'
import { useModulesStore } from '@/stores/modules'
import type { ModuleDto } from '@/api/modules'
import { useToast } from '@/composables/useToast'

const { t } = useI18n()
const authStore = useAuthStore()
const modulesStore = useModulesStore()
const toast = useToast()
const viewMode = ref<'table' | 'graph'>('table')
const isScanning = ref(false)
const showInstallModal = ref(false)
const selectedFile = ref<File | null>(null)
const isInstalling = ref(false)
const copiedId = ref(false)

onMounted(() => {
  modulesStore.fetchModules()
})

async function copyModuleId(id: string) {
  try {
    await navigator.clipboard.writeText(id)
    copiedId.value = true
    toast.info('ID скопирован в буфер')
    setTimeout(() => {
      copiedId.value = false
    }, 2000)
  } catch (err: any) {
    console.error('Failed to copy ID:', err)
    toast.error('Не удалось скопировать ID')
  }
}

function handleMetricClick(status: 'all' | 'active' | 'disabled') {
  if (modulesStore.filter === status && status !== 'all') {
    modulesStore.setFilter('all')
  } else {
    modulesStore.setFilter(status)
  }
}

async function handleScan() {
  isScanning.value = true
  try {
    await modulesStore.fetchModules()
    toast.success('Модули синхронизированы')
  } catch (err: any) {
    toast.error(err?.message || 'Ошибка синхронизации модулей')
  } finally {
    setTimeout(() => {
      isScanning.value = false
    }, 600)
  }
}

async function handleToggle(mod: ModuleDto) {
  try {
    await modulesStore.toggleModule(mod.id, !mod.is_active)
    toast.success(mod.is_active ? t('common.disabled') : t('common.active'))
  } catch (err: any) {
    console.error('Failed to toggle module:', err)
    toast.error(err?.message || 'Failed to toggle module')
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
    toast.success('Модуль успешно установлен')
    showInstallModal.value = false
    selectedFile.value = null
  } catch (err: any) {
    toast.error(err?.message || 'Ошибка установки модуля')
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

const filterOptions = computed(() => [
  { value: 'all', label: t('modules.all') },
  { value: 'active', label: t('modules.activeStatus') },
  { value: 'disabled', label: t('modules.disabledStatus') }
])
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)] select-none">
    <!-- Top Settings Subnavigation -->
    <SettingsNav />

    <!-- Main Content Area with Aside -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="relative z-10 p-lg flex gap-lg h-full w-full">
        <!-- Submenu Sidebar -->
        <aside class="w-nav-width shrink-0 hidden md:flex flex-col gap-sm border-r border-outline-variant/60 pr-md">
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
              {{ t('dashboard.platformModules') }}
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
          <PageHeader
            :title="t('modules.title')"
            :subtitle="t('modules.subtitle')"
            icon="view_module"
          >
            <template #actions>
              <AppButton
                variant="outline"
                size="sm"
                icon="refresh"
                :loading="isScanning"
                @click="handleScan"
              >
                {{ t('modules.scanNewModules') }}
              </AppButton>
              <AppButton
                v-if="authStore.canManageModules"
                variant="primary"
                size="sm"
                icon="add"
                @click="showInstallModal = true"
              >
                {{ t('modules.installModule') }}
              </AppButton>
            </template>
          </PageHeader>

          <!-- Metric Cards -->
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-md">
            <!-- Total Modules Card (Clickable Filter) -->
            <div
              class="bg-surface-container-low border p-lg rounded-xl shadow-card-dark flex items-start justify-between cursor-pointer transition-all hover:border-outline-variant hover:bg-surface-container/80"
              :class="modulesStore.filter === 'all' ? 'border-primary-fixed-dim/40 ring-1 ring-primary-fixed-dim/30' : 'border-outline-variant/60'"
              @click="handleMetricClick('all')"
            >
              <div>
                <p class="text-[11px] font-mono text-on-surface-variant uppercase tracking-widest mb-sm">
                  {{ t('modules.totalModules') }}
                </p>
                <p class="text-[32px] leading-none font-mono font-bold text-on-surface">
                  {{ modulesStore.totalCount }}
                </p>
              </div>
              <div class="w-9 h-9 rounded-lg bg-surface-container-highest flex items-center justify-center text-on-surface-variant/70 border border-outline-variant/40">
                <span class="material-symbols-outlined text-lg">widgets</span>
              </div>
            </div>

            <!-- Active Modules Card (Clickable Filter) -->
            <div
              class="bg-surface-container-low border p-lg rounded-xl shadow-card-dark flex items-start justify-between cursor-pointer transition-all hover:border-tertiary-fixed-dim/40 hover:bg-surface-container/80"
              :class="modulesStore.filter === 'active' ? 'border-tertiary-fixed-dim/60 ring-1 ring-tertiary-fixed-dim/40' : 'border-outline-variant/60'"
              @click="handleMetricClick('active')"
            >
              <div>
                <p class="text-[11px] font-mono text-tertiary-fixed-dim uppercase tracking-widest mb-sm">
                  {{ t('modules.active') }}
                </p>
                <p class="text-[32px] leading-none font-mono font-bold text-tertiary-fixed-dim">
                  {{ modulesStore.activeCount }}
                </p>
              </div>
              <div class="w-9 h-9 rounded-lg bg-tertiary-fixed-dim/10 flex items-center justify-center text-tertiary-fixed-dim border border-tertiary-fixed-dim/30">
                <span class="material-symbols-outlined text-lg">check_circle</span>
              </div>
            </div>

            <!-- Disabled Modules Card (Clickable Filter) -->
            <div
              class="bg-surface-container-low border p-lg rounded-xl shadow-card-dark flex items-start justify-between cursor-pointer transition-all hover:border-outline-variant hover:bg-surface-container/80"
              :class="modulesStore.filter === 'disabled' ? 'border-error/40 ring-1 ring-error/30' : 'border-outline-variant/60'"
              @click="handleMetricClick('disabled')"
            >
              <div>
                <p class="text-[11px] font-mono text-on-surface-variant uppercase tracking-widest mb-sm">
                  {{ t('modules.disabled') }}
                </p>
                <p class="text-[32px] leading-none font-mono font-bold text-on-surface">
                  {{ modulesStore.disabledCount }}
                </p>
              </div>
              <div class="w-9 h-9 rounded-lg bg-surface-container-highest flex items-center justify-center text-on-surface-variant/70 border border-outline-variant/40">
                <span class="material-symbols-outlined text-lg">block</span>
              </div>
            </div>

            <!-- Runtime Info Card -->
            <div class="bg-surface-container-low border border-outline-variant/60 p-lg rounded-xl shadow-card-dark flex items-start justify-between">
              <div>
                <p class="text-[11px] font-mono text-primary-fixed-dim uppercase tracking-widest mb-sm">
                  {{ t('modules.runtime') }}
                </p>
                <p class="text-xl leading-tight font-mono font-bold text-primary-fixed-dim">
                  {{ t('modules.runtimeValue') }}
                </p>
                <span class="text-[10px] font-mono text-on-surface-variant/70 block mt-1">AetherCore IPC Engine</span>
              </div>
              <div class="w-9 h-9 rounded-lg bg-primary-fixed-dim/10 flex items-center justify-center text-primary-fixed-dim border border-primary-fixed-dim/30 shrink-0">
                <span class="material-symbols-outlined text-lg">memory</span>
              </div>
            </div>
          </div>

          <!-- Main Layout: Registry + Side Details -->
          <div class="flex flex-col lg:flex-row gap-lg">
            <!-- Left: Module Registry Card -->
            <BaseCard
              :title="t('modules.moduleRegistry')"
              :subtitle="t('modules.moduleRegistryDesc')"
              icon="layers"
              :no-padding="true"
              class="flex-1"
            >
              <template #headerActions>
                <!-- Search Input -->
                <div class="relative w-44 sm:w-56">
                  <span class="material-symbols-outlined absolute left-2.5 top-1/2 -translate-y-1/2 text-on-surface-variant text-[16px] pointer-events-none">
                    search
                  </span>
                  <input
                    v-model="modulesStore.searchQuery"
                    type="text"
                    :placeholder="t('modules.searchPlaceholder')"
                    class="w-full bg-surface-container-highest border border-outline-variant/60 rounded-lg pl-8 pr-7 py-1 text-xs text-on-surface placeholder:text-on-surface-variant/60 focus:outline-none focus:border-primary-fixed-dim transition-colors"
                  />
                  <button
                    v-if="modulesStore.searchQuery"
                    type="button"
                    class="absolute right-2 top-1/2 -translate-y-1/2 text-on-surface-variant hover:text-on-surface"
                    @click="modulesStore.setSearchQuery('')"
                  >
                    <span class="material-symbols-outlined text-[14px]">close</span>
                  </button>
                </div>

                <!-- View Switcher -->
                <div class="flex bg-surface-container-highest border border-outline-variant/60 rounded-lg p-0.5">
                  <button
                    type="button"
                    class="px-3 py-1 rounded-md text-xs font-semibold flex items-center gap-1.5 transition-all cursor-pointer"
                    :class="viewMode === 'table'
                      ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm font-bold'
                      : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50'"
                    @click="viewMode = 'table'"
                  >
                    <span class="material-symbols-outlined text-[16px]">table_rows</span>
                    {{ t('modules.tableView') }}
                  </button>
                  <button
                    type="button"
                    class="px-3 py-1 rounded-md text-xs font-semibold flex items-center gap-1.5 transition-all cursor-pointer"
                    :class="viewMode === 'graph'
                      ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm font-bold'
                      : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50'"
                    @click="viewMode = 'graph'"
                  >
                    <span class="material-symbols-outlined text-[16px]">account_tree</span>
                    {{ t('modules.topologyGraph') }}
                  </button>
                </div>

                <!-- Status Filter Select -->
                <div class="w-36">
                  <BaseSelect
                    :model-value="modulesStore.filter"
                    :options="filterOptions"
                    size="sm"
                    @update:model-value="(val) => modulesStore.setFilter(val)"
                  />
                </div>
              </template>

              <!-- Table View -->
              <div v-if="viewMode === 'table'" class="overflow-x-auto">
                <table class="w-full text-left border-collapse">
                  <thead class="bg-surface-container-highest/60 text-[10px] text-on-surface-variant uppercase font-bold tracking-wider border-b border-outline-variant/60">
                    <tr>
                      <th class="py-3 px-md w-1/4">
                        {{ t('modules.moduleName') }}
                      </th>
                      <th class="py-3 px-md w-1/5">
                        {{ t('modules.moduleTypeCol') }}
                      </th>
                      <th class="py-3 px-md w-1/6">
                        {{ t('modules.version') }}
                      </th>
                      <th class="py-3 px-md w-1/6">
                        {{ t('modules.status') }}
                      </th>
                      <th v-if="authStore.canManageModules" class="py-3 px-md text-right">
                        {{ t('modules.actions') }}
                      </th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-outline-variant/30 text-xs">
                    <tr
                      v-for="mod in modulesStore.filteredModules"
                      :key="mod.id"
                      class="hover:bg-surface-variant/20 transition-colors cursor-pointer"
                      :class="{ 'bg-surface-variant/40 ring-1 ring-inset ring-primary-fixed-dim/30': modulesStore.selectedModule?.id === mod.id }"
                      @click="modulesStore.selectModule(mod)"
                    >
                      <td class="py-md px-md">
                        <div class="flex items-center gap-3">
                          <div class="w-8 h-8 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                            <span class="material-symbols-outlined text-[18px]">extension</span>
                          </div>
                          <div>
                            <p class="font-bold text-on-surface text-sm" :title="mod.name">{{ mod.name }}</p>
                            <p class="text-[11px] font-mono text-on-surface-variant">{{ mod.id }}</p>
                          </div>
                        </div>
                      </td>
                      <td class="py-md px-md font-mono text-xs text-on-surface-variant">
                        {{ mod.manifest?.ui?.type || 'WASM Core' }}
                      </td>
                      <td class="py-md px-md font-mono text-xs text-on-surface-variant">
                        v{{ mod.version }}
                      </td>
                      <td class="py-md px-md">
                        <StatusBadge
                          :variant="mod.is_active ? 'success' : 'neutral'"
                          :dot="true"
                          :pulse="mod.is_active"
                          size="xs"
                        >
                          {{ mod.is_active ? t('modules.activeStatus') : t('modules.disabledStatus') }}
                        </StatusBadge>
                      </td>
                      <td v-if="authStore.canManageModules" class="py-md px-md text-right">
                        <AppButton
                          :variant="mod.is_active ? 'danger' : 'tertiary'"
                          size="xs"
                          :icon="mod.is_active ? 'power_off' : 'play_arrow'"
                          :loading="modulesStore.togglingId === mod.id"
                          @click.stop="handleToggle(mod)"
                        >
                          {{ mod.is_active ? t('modules.disable') : t('modules.enable') }}
                        </AppButton>
                      </td>
                    </tr>

                    <tr v-if="modulesStore.filteredModules.length === 0">
                      <td class="py-xl px-md text-center text-sm text-on-surface-variant" :colspan="authStore.canManageModules ? 5 : 4">
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
                  <!-- Central Hub: AetherCore Bus -->
                  <div class="px-6 py-3 rounded-xl bg-surface-container-highest border-2 border-primary-fixed-dim text-primary-fixed-dim shadow-glow-primary-md flex items-center gap-3">
                    <span class="material-symbols-outlined text-2xl animate-pulse">hub</span>
                    <div class="text-left">
                      <div class="text-xs font-bold font-mono uppercase">AetherCore Message Bus</div>
                      <div class="text-[10px] text-on-surface-variant">IPC / WASM Host Runtime</div>
                    </div>
                  </div>

                  <!-- Connected Module Nodes -->
                  <div class="grid grid-cols-2 sm:grid-cols-3 gap-6 w-full">
                    <div
                      v-for="mod in modulesStore.filteredModules"
                      :key="mod.id"
                      class="p-3 rounded-lg border flex flex-col items-center gap-2 cursor-pointer transition-all hover:scale-105"
                      :class="[
                        mod.is_active
                          ? 'bg-surface-container border-tertiary-fixed-dim/40 shadow-glow-tertiary-sm'
                          : 'bg-surface-container/60 border-outline-variant opacity-60',
                        modulesStore.selectedModule?.id === mod.id ? 'ring-2 ring-primary-fixed-dim' : ''
                      ]"
                      @click="modulesStore.selectModule(mod)"
                    >
                      <div class="w-10 h-10 rounded-full flex items-center justify-center" :class="mod.is_active ? 'bg-tertiary-fixed-dim/20 text-tertiary-fixed-dim' : 'bg-surface-variant text-on-surface-variant'">
                        <span class="material-symbols-outlined text-xl">extension</span>
                      </div>
                      <span class="text-xs font-bold text-on-surface">{{ mod.name }}</span>
                      <div class="flex items-center gap-1">
                        <span class="text-[10px] font-mono text-on-surface-variant">v{{ mod.version }}</span>
                        <span
                          class="w-1.5 h-1.5 rounded-full inline-block"
                          :class="mod.is_active ? 'bg-tertiary-fixed-dim' : 'bg-on-surface-variant/40'"
                        />
                      </div>
                    </div>

                    <!-- Host System Adapter Node -->
                    <div class="p-3 rounded-lg border border-outline-variant bg-surface-container flex flex-col items-center gap-2">
                      <div class="w-10 h-10 rounded-full bg-surface-variant flex items-center justify-center text-primary-fixed-dim">
                        <span class="material-symbols-outlined text-xl">router</span>
                      </div>
                      <span class="text-xs font-bold text-on-surface">Network Driver</span>
                      <span class="text-[10px] font-mono text-on-surface-variant">Sys Socket</span>
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
            </BaseCard>

            <!-- Right: Module Details Card -->
            <div class="w-full lg:w-96 shrink-0">
              <BaseCard
                v-if="modulesStore.selectedModule"
                :no-padding="false"
              >
                <!-- Custom Header without Text Truncation Issues -->
                <template #header>
                  <div class="flex items-start gap-3 w-full min-w-0">
                    <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0 mt-0.5">
                      <span class="material-symbols-outlined text-xl">extension</span>
                    </div>
                    <div class="flex-1 min-w-0">
                      <div class="flex items-start justify-between gap-2">
                        <h2
                          class="font-title-sm font-bold text-on-surface text-sm break-words leading-tight"
                          :title="modulesStore.selectedModule.name"
                        >
                          {{ modulesStore.selectedModule.name }}
                        </h2>
                        <StatusBadge
                          :variant="modulesStore.selectedModule.is_active ? 'success' : 'neutral'"
                          :dot="true"
                          :pulse="modulesStore.selectedModule.is_active"
                          size="xs"
                        >
                          {{ modulesStore.selectedModule.is_active ? t('modules.activeStatus') : t('modules.disabledStatus') }}
                        </StatusBadge>
                      </div>

                      <div class="flex items-center gap-2 mt-1.5 flex-wrap">
                        <span class="text-[11px] font-mono text-on-surface-variant/90">
                          {{ modulesStore.selectedModule.id }}
                        </span>
                        <button
                          type="button"
                          class="inline-flex items-center gap-1 text-[10px] text-primary-fixed-dim hover:text-on-surface transition-colors cursor-pointer"
                          :title="t('modules.copyId')"
                          @click="copyModuleId(modulesStore.selectedModule.id)"
                        >
                          <span class="material-symbols-outlined text-[14px]">
                            {{ copiedId ? 'check' : 'content_copy' }}
                          </span>
                          <span v-if="copiedId" class="text-[9px] text-tertiary-fixed-dim font-mono">{{ t('modules.copied') }}</span>
                        </button>
                        <span class="px-1.5 py-0.5 bg-surface-container-highest border border-outline-variant/60 rounded text-[10px] font-mono text-on-surface-variant ml-auto">
                          v{{ modulesStore.selectedModule.version }}
                        </span>
                      </div>
                    </div>
                  </div>
                </template>

                <div class="flex flex-col gap-md">
                  <div>
                    <label class="text-[10px] font-mono text-on-surface-variant uppercase block mb-1">
                      {{ t('modules.description') }}
                    </label>
                    <p class="text-xs text-on-surface leading-relaxed">
                      {{ modulesStore.selectedModule.manifest?.description || t('modules.noDescription') }}
                    </p>
                  </div>

                  <div class="grid grid-cols-2 gap-2">
                    <div>
                      <label class="text-[10px] font-mono text-on-surface-variant uppercase block mb-1">
                        {{ t('modules.author') }}
                      </label>
                      <p class="text-xs text-on-surface font-mono">
                        {{ modulesStore.selectedModule.manifest?.author || 'AetherCore Team' }}
                      </p>
                    </div>

                    <div v-if="modulesStore.selectedModule.manifest?.entrypoint">
                      <label class="text-[10px] font-mono text-on-surface-variant uppercase block mb-1">
                        {{ t('modules.entrypoint') }}
                      </label>
                      <p class="text-xs text-on-surface font-mono truncate" :title="modulesStore.selectedModule.manifest.entrypoint">
                        {{ modulesStore.selectedModule.manifest.entrypoint }}
                      </p>
                    </div>
                  </div>

                  <div>
                    <label class="text-[10px] font-mono text-on-surface-variant uppercase block mb-1">
                      {{ t('modules.permissions') }}
                    </label>
                    <div class="flex flex-wrap gap-1.5">
                      <span
                        v-for="p in modulesStore.selectedModule.manifest?.permissions || ['network.listen', 'storage.kv', 'events.publish']"
                        :key="formatPermission(p)"
                        :title="getPermissionTitle(p)"
                        class="px-2 py-0.5 bg-surface-variant border border-outline-variant/60 text-[10px] font-mono text-primary-fixed-dim rounded"
                      >
                        {{ formatPermission(p) }}
                      </span>
                    </div>
                  </div>
                </div>

                <template #footer v-if="authStore.canManageModules">
                  <AppButton
                    :variant="modulesStore.selectedModule.is_active ? 'danger' : 'tertiary'"
                    size="md"
                    :block="true"
                    :loading="modulesStore.togglingId === modulesStore.selectedModule.id"
                    :icon="modulesStore.selectedModule.is_active ? 'power_off' : 'play_arrow'"
                    @click="handleToggle(modulesStore.selectedModule)"
                  >
                    {{ modulesStore.selectedModule.is_active ? t('modules.disable') : t('modules.enable') }}
                  </AppButton>
                </template>
              </BaseCard>

              <!-- Empty State -->
              <div
                v-else
                class="bg-surface-container-low border border-outline-variant rounded-lg p-xl flex flex-col items-center justify-center text-center shadow-card-dark min-h-[180px] gap-2"
              >
                <div class="w-12 h-12 rounded-lg bg-surface-container-highest/60 border border-outline-variant/50 flex items-center justify-center text-on-surface-variant/40 mb-1">
                  <span class="material-symbols-outlined text-2xl">extension</span>
                </div>
                <p class="text-xs font-bold text-on-surface">
                  {{ t('modules.selectModulePrompt') }}
                </p>
                <p class="text-[11px] text-on-surface-variant max-w-[200px]">
                  {{ t('modules.selectModuleHint') }}
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- Modal: Install Module -->
    <BaseModal
      v-model="showInstallModal"
      :title="t('modules.uploadWasmTitle')"
      :subtitle="t('modules.uploadWasmDesc')"
      icon="add_box"
      max-width="max-w-lg"
    >
      <!-- Drop Zone -->
      <div
        class="border-2 border-dashed border-outline-variant hover:border-primary-fixed-dim/60 rounded-lg p-xl flex flex-col items-center justify-center text-center cursor-pointer transition-colors bg-surface-container-lowest/50 my-2"
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
        <p class="text-[10px] text-on-surface-variant font-mono">
          {{ selectedFile ? `${(selectedFile.size / 1024).toFixed(1)} KB` : t('modules.browseFiles') }}
        </p>
      </div>

      <template #footer>
        <AppButton
          variant="ghost"
          size="sm"
          @click="showInstallModal = false"
        >
          {{ t('modules.cancel') }}
        </AppButton>
        <AppButton
          variant="primary"
          size="sm"
          :disabled="!selectedFile"
          :loading="isInstalling"
          @click="handleInstall"
        >
          {{ isInstalling ? t('modules.installing') : t('modules.install') }}
        </AppButton>
      </template>
    </BaseModal>
  </div>
</template>
