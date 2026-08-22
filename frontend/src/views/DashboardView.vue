<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from '@/i18n'
import { useModulesStore } from '@/stores/modules'
import {
  AppButton,
  StatusBadge
} from '@/components/common'

const { t } = useI18n()
const router = useRouter()
const modulesStore = useModulesStore()

const lastUpdated = ref('22:27:59')
const activeDesktop = ref('main')

onMounted(async () => {
  try {
    await modulesStore.fetchModules()
  } catch (e) {
    console.warn('Failed to load modules:', e)
  }
  lastUpdated.value = new Date().toLocaleTimeString()
})

async function handleRefresh() {
  try {
    await modulesStore.fetchModules()
  } catch (e) {
    console.warn('Failed to refresh modules:', e)
  }
  lastUpdated.value = new Date().toLocaleTimeString()
}
</script>

<template>
  <!-- BEGIN: MainDashboardCanvas -->
  <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative flex flex-col select-none">
    <!-- Dot Grid Background -->
    <div class="relative z-10 p-lg flex flex-col h-full flex-1">
      <!-- Header Row -->
      <div class="flex justify-between items-start mb-lg">
        <div class="flex items-start gap-md">
          <span class="material-symbols-outlined text-primary-fixed-dim text-[28px] mt-1">grid_view</span>
          <div>
            <h2 class="text-display-lg font-display-lg text-on-surface leading-none">{{ t('dashboard.title') }}</h2>
            <p class="text-body-base font-body-base text-on-surface-variant mt-2">{{ t('dashboard.subtitle') }}</p>
          </div>
        </div>
        <div class="flex items-center gap-sm">
          <button
            type="button"
            class="flex items-center gap-sm px-md py-2 bg-primary-fixed-dim/10 border border-primary-fixed-dim text-primary-fixed-dim font-title-sm text-sm hover:bg-primary-fixed-dim/20 shadow-glow-primary-sm transition-all rounded-lg cursor-pointer active:scale-95"
          >
            <span class="material-symbols-outlined text-[18px]">add</span>
            {{ t('dashboard.addWidget') }}
          </button>
          <button
            type="button"
            class="flex items-center gap-sm px-md py-2 border border-outline-variant text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all font-title-sm text-sm rounded-lg cursor-pointer active:scale-95"
          >
            <span class="material-symbols-outlined text-[18px]">dashboard_customize</span>
            {{ t('dashboard.customizeDashboard') }}
          </button>
        </div>
      </div>

      <!-- Desktop Switcher -->
      <div class="flex items-center gap-md mb-lg">
        <button
          type="button"
          class="flex items-center gap-sm px-md py-2 border border-primary-fixed-dim bg-primary-fixed-dim/10 text-primary-fixed-dim font-title-sm text-sm shadow-glow-primary-sm transition-all rounded-lg cursor-pointer"
          :class="{ 'ring-1 ring-primary-fixed-dim': activeDesktop === 'main' }"
          @click="activeDesktop = 'main'"
        >
          <span class="material-symbols-outlined text-[18px]">monitor</span>
          {{ t('dashboard.mainDesktop') }}
        </button>
        <button
          type="button"
          class="flex items-center gap-sm px-md py-2 border border-outline-variant border-dashed text-on-surface-variant hover:text-primary-fixed-dim hover:border-primary-fixed-dim/50 transition-all font-title-sm text-sm rounded-lg cursor-pointer active:scale-95"
        >
          <span class="material-symbols-outlined text-[18px]">add</span>
          {{ t('dashboard.newDesktop') }}
        </button>
      </div>

      <!-- Widgets Area -->
      <div class="flex-1 w-full relative border border-outline-variant/50 rounded-lg overflow-hidden overflow-y-auto h-full p-4">
        <div
          class="absolute inset-0 pointer-events-none opacity-40 z-0"
          style="background-image: radial-gradient(circle at 1px 1px, rgba(115, 212, 232, 0.15) 1px, transparent 0px); background-size: 24px 24px;"
        ></div>

        <!-- Platform Modules Widget -->
        <div class="widget-card rounded-lg flex flex-col shadow-card-dark overflow-hidden border border-outline-variant bg-surface-container relative z-10 w-full max-w-[420px]">
          <!-- Widget Header -->
          <div class="px-md py-sm border-b border-outline-variant flex items-center justify-between bg-surface-container-high/50">
            <div class="flex items-center gap-sm">
              <span class="material-symbols-outlined text-primary-fixed-dim text-[18px]">grid_view</span>
              <h3 class="font-title-sm text-sm text-on-surface">{{ t('dashboard.platformModules') }}</h3>
            </div>
            <div class="flex items-center gap-md">
              <span class="px-2 py-0.5 rounded text-[10px] font-label-caps uppercase tracking-wider bg-primary-fixed-dim/10 text-primary-fixed-dim border border-primary-fixed-dim/20">
                {{ t('dashboard.systemBadge') }}
              </span>
              <button
                type="button"
                class="text-on-surface-variant hover:text-primary-fixed-dim transition-colors flex items-center justify-center p-1 rounded hover:bg-surface-variant/50 cursor-pointer"
                @click="handleRefresh"
                title="Refresh"
              >
                <span class="material-symbols-outlined text-[18px]">refresh</span>
              </button>
            </div>
          </div>

          <!-- Widget Body -->
          <div class="p-md flex flex-col gap-sm">
            <!-- Status Summary row -->
            <div class="flex items-center justify-between p-sm rounded-lg bg-surface-container-lowest border border-outline-variant/30">
              <div class="flex items-center gap-sm">
                <span class="material-symbols-outlined text-primary-fixed-dim text-[18px]">assignment</span>
                <span class="font-body-base text-sm text-on-surface">{{ t('dashboard.moduleSummary') }}</span>
              </div>
              <span class="px-2 py-0.5 rounded text-[10px] font-label-caps uppercase tracking-wider bg-tertiary-fixed-dim/10 text-tertiary-fixed-dim border border-tertiary-fixed-dim/20 flex items-center gap-1">
                {{ t('dashboard.loadedCount', { active: modulesStore.activeCount, total: modulesStore.totalCount }) }}
              </span>
            </div>

            <!-- Module list or Empty State -->
            <div v-if="modulesStore.modules.length > 0" class="flex flex-col gap-2 my-1">
              <div
                v-for="mod in modulesStore.modules"
                :key="mod.id"
                class="flex items-center justify-between p-2.5 rounded-lg bg-surface-container-low border border-outline-variant/30 hover:border-primary-fixed-dim/50 transition-all cursor-pointer"
                @click="router.push('/modules')"
              >
                <div class="flex items-center gap-3">
                  <span class="material-symbols-outlined text-primary-fixed-dim text-[18px]">extension</span>
                  <div>
                    <p class="text-xs font-bold text-on-surface">{{ mod.name }}</p>
                    <p class="text-[10px] text-on-surface-variant font-body-mono">v{{ mod.version }}</p>
                  </div>
                </div>
                <span
                  class="px-2 py-0.5 rounded text-[10px] font-label-caps uppercase tracking-wider"
                  :class="mod.is_active ? 'bg-tertiary-fixed-dim/10 text-tertiary-fixed-dim border border-tertiary-fixed-dim/20' : 'bg-surface-variant text-on-surface-variant border border-outline-variant/40'"
                >
                  {{ mod.is_active ? t('common.active') : t('common.disabled') }}
                </span>
              </div>
            </div>

            <!-- Empty State -->
            <div v-else class="flex items-center justify-center py-20 my-md">
              <p class="text-on-surface-variant text-sm font-body-base">{{ t('dashboard.noModules') }}</p>
            </div>
          </div>

          <!-- Widget Footer -->
          <div class="px-md py-2 border-t border-outline-variant flex items-center justify-between bg-surface-container-low/50">
            <span class="text-xs font-body-mono text-outline">{{ t('common.updatedAt') }}: {{ lastUpdated }}</span>
            <button
              type="button"
              class="flex items-center gap-xs text-xs font-title-sm text-primary-fixed-dim hover:text-primary-fixed-dim/80 transition-colors rounded-lg cursor-pointer"
              @click="router.push('/modules')"
            >
              {{ t('dashboard.manageModules') }}
              <span class="material-symbols-outlined text-[16px]">settings</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </main>
  <!-- END: MainDashboardCanvas -->
</template>
