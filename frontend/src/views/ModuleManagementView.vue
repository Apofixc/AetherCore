<script setup lang="ts">
import { ref, onMounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import { useI18n } from '@/i18n'
import { useModulesStore } from '@/stores/modules'
import type { ModuleDto } from '@/api/modules'

const { t } = useI18n()
const modulesStore = useModulesStore()
const viewMode = ref<'table' | 'graph'>('table')

onMounted(() => {
  modulesStore.fetchModules()
})

async function handleToggle(mod: ModuleDto) {
  try {
    await modulesStore.toggleModule(mod.id, !mod.is_active)
  } catch (err) {
    console.error('Failed to toggle module:', err)
  }
}
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)] select-none">
    <!-- Top Settings Subnavigation -->
    <SettingsNav />

    <!-- Main Content Area with Aside -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="relative z-10 p-lg flex gap-lg h-full">
        <!-- Sub-Rail Aside -->
        <aside class="w-nav-width shrink-0 hidden md:flex flex-col gap-sm border-r border-outline-variant pr-md">
          <div class="px-md mb-sm">
            <div class="flex flex-col gap-xs">
              <router-link
                to="/modules"
                class="flex items-center gap-md px-md py-sm rounded-lg bg-primary-fixed-dim/10 border-l-2 border-primary-fixed-dim text-primary-fixed-dim font-bold transition-all duration-200"
              >
                <span class="material-symbols-outlined" style="font-variation-settings: 'FILL' 1;">view_module</span>
                <h3 class="text-label-caps font-label-caps text-primary-fixed-dim uppercase tracking-wider">
                  Module Management
                </h3>
              </router-link>
            </div>
          </div>

          <div class="px-md mt-md">
            <h3 class="text-label-caps font-label-caps text-on-surface-variant uppercase tracking-wider mb-sm">
              Dynamic Modules
            </h3>
            <div class="flex flex-col gap-xs">
              <a href="#" class="flex items-center gap-md px-md py-sm rounded-lg text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all" @click.prevent>
                <span class="material-symbols-outlined">analytics</span>
                Data Processor
              </a>
              <a href="#" class="flex items-center gap-md px-md py-sm rounded-lg text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all" @click.prevent>
                <span class="material-symbols-outlined">folder</span>
                File Explorer
              </a>
            </div>
          </div>
        </aside>

        <!-- Right Content Body -->
        <div class="flex-1 flex flex-col gap-lg mx-auto w-full pb-xl">
          <!-- View Header -->
          <div class="flex items-center justify-between">
            <div>
              <h1 class="font-display-lg text-display-lg text-on-surface font-bold">Module Management</h1>
              <p class="text-sm text-on-surface-variant mt-1">Monitor and control system-level service modules.</p>
            </div>
            <div class="flex items-center gap-3">
              <button
                type="button"
                class="bg-surface-container-low hover:bg-surface-variant text-on-surface border border-outline-variant px-4 py-2 rounded-lg text-sm font-semibold transition-colors flex items-center gap-2 cursor-pointer"
                @click="modulesStore.fetchModules"
              >
                <span class="material-symbols-outlined text-[18px]">refresh</span>
                Scan for New Modules
              </button>
              <button
                type="button"
                class="bg-primary-fixed-dim hover:bg-primary-fixed-dim/90 text-on-primary-fixed border border-primary-fixed-dim px-4 py-2 rounded-lg text-sm font-semibold transition-colors flex items-center gap-2 cursor-pointer"
              >
                <span class="material-symbols-outlined text-[18px]">add</span>
                Install Module
              </button>
            </div>
          </div>

          <!-- 4 Stat Cards -->
          <div class="grid grid-cols-4 gap-md">
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.2)]">
              <p class="text-label-caps font-label-caps text-on-surface-variant uppercase tracking-widest mb-sm">
                Total Modules
              </p>
              <p class="text-[32px] leading-none font-label-mono-sm font-bold text-on-surface">
                {{ modulesStore.totalCount }}
              </p>
            </div>

            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.2)]">
              <p class="text-label-caps font-label-caps text-tertiary-fixed-dim uppercase tracking-widest mb-sm">
                Active
              </p>
              <p class="text-[32px] leading-none font-label-mono-sm font-bold text-tertiary-fixed-dim">
                {{ modulesStore.activeCount }}
              </p>
            </div>

            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.2)]">
              <p class="text-label-caps font-label-caps text-on-surface-variant uppercase tracking-widest mb-sm">
                Disabled
              </p>
              <p class="text-[32px] leading-none font-label-mono-sm font-bold text-on-surface">
                {{ modulesStore.disabledCount }}
              </p>
            </div>

            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.2)] border-primary-fixed-dim/30 shadow-[0_0_10px_rgba(115,212,232,0.1)]">
              <p class="text-label-caps font-label-caps text-primary-fixed-dim uppercase tracking-widest mb-sm">
                Module Type
              </p>
              <p class="text-[32px] leading-none font-label-mono-sm font-bold text-primary-fixed-dim">
                {{ modulesStore.modules.length > 0 ? 'WASM' : '0' }}
              </p>
            </div>
          </div>

          <!-- Main Table + Side Details Container -->
          <div class="flex gap-lg">
            <!-- Table Registry Card -->
            <div class="flex-1 bg-surface-container-low border border-outline-variant rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.2)] shadow-[0_0_15px_rgba(115,212,232,0.05)] border-primary-fixed-dim/20 flex flex-col">
              <div class="p-md border-b border-outline-variant flex items-center justify-between bg-surface-container">
                <div class="flex items-center gap-lg">
                  <h2 class="font-title-sm text-title-sm text-on-surface font-bold">Module Registry</h2>
                  <div class="flex bg-surface-dim rounded-lg p-1 border border-outline-variant/50">
                    <button
                      type="button"
                      class="px-3 py-1.5 rounded text-sm font-semibold flex items-center gap-2 transition-all cursor-pointer"
                      :class="viewMode === 'table' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'text-on-surface-variant hover:text-on-surface'"
                      @click="viewMode = 'table'"
                    >
                      <span class="material-symbols-outlined text-[18px]">table_rows</span>
                      Table View
                    </button>
                    <button
                      type="button"
                      class="px-3 py-1.5 rounded text-sm font-semibold flex items-center gap-2 transition-all cursor-pointer"
                      :class="viewMode === 'graph' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'text-on-surface-variant hover:text-on-surface'"
                      @click="viewMode = 'graph'"
                    >
                      <span class="material-symbols-outlined text-[18px]">account_tree</span>
                      Topology Graph
                    </button>
                  </div>
                </div>

                <!-- Filters -->
                <div class="flex items-center gap-md">
                  <span class="text-xs font-bold text-on-surface-variant uppercase tracking-wider">FILTER:</span>
                  <div class="flex bg-surface-dim rounded-lg p-1 border border-outline-variant/50">
                    <button
                      type="button"
                      class="px-4 py-1 rounded text-xs font-bold uppercase tracking-wider transition-all cursor-pointer"
                      :class="modulesStore.filter === 'all' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'text-on-surface-variant hover:text-on-surface'"
                      @click="modulesStore.setFilter('all')"
                    >
                      All
                    </button>
                    <button
                      type="button"
                      class="px-4 py-1 rounded text-xs font-bold uppercase tracking-wider transition-all cursor-pointer"
                      :class="modulesStore.filter === 'active' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'text-on-surface-variant hover:text-on-surface'"
                      @click="modulesStore.setFilter('active')"
                    >
                      ACTIVE
                    </button>
                    <button
                      type="button"
                      class="px-4 py-1 rounded text-xs font-bold uppercase tracking-wider transition-all cursor-pointer"
                      :class="modulesStore.filter === 'disabled' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'text-on-surface-variant hover:text-on-surface'"
                      @click="modulesStore.setFilter('disabled')"
                    >
                      DISABLED
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
                        MODULE NAME
                      </th>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest w-1/5">
                        MODULE TYPE
                      </th>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest w-1/6">
                        VERSION
                      </th>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest w-1/6">
                        STATUS
                      </th>
                      <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase tracking-widest text-right">
                        ACTIONS
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="mod in modulesStore.filteredModules"
                      :key="mod.id"
                      class="border-b border-outline-variant/20 hover:bg-surface-container-highest/40 transition-colors cursor-pointer"
                      :class="{ 'bg-surface-container-highest/60': modulesStore.selectedModule?.id === mod.id }"
                      @click="modulesStore.selectModule(mod)"
                    >
                      <td class="py-md px-md">
                        <div class="flex items-center gap-2">
                          <span class="material-symbols-outlined text-primary-fixed-dim text-sm">extension</span>
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
                          :class="mod.is_active ? 'bg-tertiary-fixed-dim/15 text-tertiary-fixed-dim border border-tertiary-fixed-dim/30' : 'bg-surface-variant text-on-surface-variant border border-outline-variant/40'"
                        >
                          {{ mod.is_active ? 'ACTIVE' : 'DISABLED' }}
                        </span>
                      </td>
                      <td class="py-md px-md text-right">
                        <button
                          type="button"
                          class="px-3 py-1 text-xs font-semibold rounded border transition-colors cursor-pointer mr-2"
                          :class="mod.is_active
                            ? 'border-error text-error hover:bg-error/10'
                            : 'border-tertiary-fixed-dim text-tertiary-fixed-dim hover:bg-tertiary-fixed-dim/10'"
                          @click.stop="handleToggle(mod)"
                        >
                          {{ mod.is_active ? 'Disable' : 'Enable' }}
                        </button>
                      </td>
                    </tr>

                    <tr v-if="modulesStore.filteredModules.length === 0">
                      <td class="py-xl px-md text-center text-sm text-on-surface-variant" colspan="5">
                        No modules found
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>

              <!-- Topology Graph Placeholder -->
              <div v-else class="p-xl flex flex-col items-center justify-center min-h-[300px] text-center">
                <span class="material-symbols-outlined text-[48px] text-primary-fixed-dim mb-md animate-pulse">account_tree</span>
                <h3 class="text-on-surface font-title-sm text-sm font-bold">WASM Module Mesh Topology</h3>
                <p class="text-xs text-on-surface-variant max-w-sm mt-1 font-body-base">
                  Interactive Node Graph displaying intra-process WASM message bus connections and channels.
                </p>
              </div>
            </div>

            <!-- Module Details Card -->
            <div class="w-80 shrink-0">
              <div
                v-if="modulesStore.selectedModule"
                class="bg-surface-container-low border border-outline-variant rounded-xl p-lg shadow-[0_0_15px_rgba(0,0,0,0.2)] flex flex-col gap-md"
              >
                <div class="flex items-center justify-between border-b border-outline-variant/40 pb-sm">
                  <h3 class="font-title-sm text-sm text-on-surface font-bold">{{ modulesStore.selectedModule.name }}</h3>
                  <span class="text-[10px] font-body-mono text-primary-fixed-dim font-bold">v{{ modulesStore.selectedModule.version }}</span>
                </div>

                <div>
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">Description</label>
                  <p class="text-xs text-on-surface">
                    {{ modulesStore.selectedModule.manifest?.description || 'No description provided.' }}
                  </p>
                </div>

                <div>
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">Author</label>
                  <p class="text-xs text-on-surface font-body-mono">
                    {{ modulesStore.selectedModule.manifest?.author || 'AetherCore Team' }}
                  </p>
                </div>

                <div>
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase block mb-1">Permissions</label>
                  <div class="flex flex-wrap gap-1">
                    <span
                      v-for="p in modulesStore.selectedModule.manifest?.permissions || ['network.listen', 'storage.kv']"
                      :key="p"
                      class="px-1.5 py-0.5 bg-surface-variant text-[10px] font-body-mono text-primary-fixed-dim rounded"
                    >
                      {{ p }}
                    </span>
                  </div>
                </div>

                <div class="mt-md pt-sm border-t border-outline-variant/40 flex justify-end gap-2">
                  <button
                    type="button"
                    class="w-full py-1.5 bg-primary-fixed-dim text-on-primary-fixed rounded-lg text-xs font-bold transition-all hover:bg-primary-fixed-dim/90 cursor-pointer"
                    @click="handleToggle(modulesStore.selectedModule)"
                  >
                    {{ modulesStore.selectedModule.is_active ? 'Disable' : 'Enable' }}
                  </button>
                </div>
              </div>

              <div
                v-else
                class="bg-surface-container-low border border-outline-variant rounded-xl p-xl flex items-center justify-center text-center shadow-[0_0_15px_rgba(0,0,0,0.2)] h-32"
              >
                <p class="text-sm text-on-surface-variant">Select a module from the list to view details</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </main>
  </div>
</template>
