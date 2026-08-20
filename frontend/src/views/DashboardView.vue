<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from '@/i18n'
import { useModulesStore } from '@/stores/modules'

const { t } = useI18n()
const router = useRouter()
const modulesStore = useModulesStore()

const lastUpdated = ref('22:02:41')
const activeDesktop = ref('main')

onMounted(async () => {
  await modulesStore.fetchModules()
  lastUpdated.value = new Date().toLocaleTimeString()
})

function handleRefresh() {
  modulesStore.fetchModules()
  lastUpdated.value = new Date().toLocaleTimeString()
}
</script>

<template>
  <main class="flex-1 main-content-scroll bg-background overflow-y-auto p-6 md:p-8 flex flex-col select-none">
    <!-- Header Row -->
    <div class="flex justify-between items-start mb-6">
      <div class="flex items-start gap-3">
        <span class="material-symbols-outlined text-primary text-[28px] mt-0.5">space_dashboard</span>
        <div>
          <h2 class="text-display-lg font-display-lg text-on-surface leading-none">{{ t('dashboard.title') }}</h2>
          <p class="text-sm font-body-base text-primary/80 font-medium mt-1.5">{{ t('dashboard.subtitle') }}</p>
        </div>
      </div>

      <div class="flex items-center gap-3">
        <button
          type="button"
          class="flex items-center gap-2 px-4 py-2 bg-primary text-on-primary font-bold text-sm rounded-lg hover:brightness-110 shadow-[0_0_15px_rgba(103,232,249,0.3)] transition-all cursor-pointer"
        >
          <span class="material-symbols-outlined text-[18px]">add</span>
          {{ t('dashboard.addWidget') }}
        </button>
        <button
          type="button"
          class="flex items-center gap-2 px-4 py-2 bg-surface-container border border-outline-variant text-on-surface hover:text-primary hover:border-primary/50 text-sm font-semibold rounded-lg transition-all cursor-pointer"
        >
          <span class="material-symbols-outlined text-[18px]">tune</span>
          {{ t('dashboard.customizeDashboard') }}
        </button>
      </div>
    </div>

    <!-- Desktop Switcher -->
    <div class="flex items-center gap-3 mb-6">
      <button
        type="button"
        class="flex items-center gap-2 px-4 py-2 border border-primary bg-primary/10 text-primary text-sm font-semibold rounded-lg shadow-[0_0_12px_rgba(103,232,249,0.15)] transition-all cursor-pointer"
        :class="{ 'opacity-100': activeDesktop === 'main' }"
        @click="activeDesktop = 'main'"
      >
        <span class="material-symbols-outlined text-[18px]">monitor</span>
        {{ t('dashboard.mainDesktop') }}
      </button>

      <button
        type="button"
        class="flex items-center gap-2 px-4 py-2 border border-outline-variant bg-surface-container/30 text-on-surface-variant hover:text-on-surface hover:border-outline text-sm font-semibold rounded-lg transition-all cursor-pointer"
      >
        <span class="material-symbols-outlined text-[18px]">add</span>
        {{ t('dashboard.newDesktop') }}
      </button>
    </div>

    <!-- Canvas / Workspace Frame Area -->
    <div class="flex-1 w-full rounded-2xl border border-outline-variant/40 bg-surface-container-lowest/50 relative overflow-hidden p-6 flex flex-col min-h-[480px]">
      <!-- Dot Grid Background inside workspace -->
      <div
        class="absolute inset-0 pointer-events-none opacity-50 dark:opacity-75"
        style="background-image: radial-gradient(circle at 1px 1px, rgba(103, 232, 249, 0.18) 1px, transparent 0px); background-size: 24px 24px;"
      ></div>

      <!-- Widgets Area inside canvas -->
      <div class="relative z-10 w-full max-w-[420px]">
        <!-- NMS Modules Widget -->
        <div class="widget-card rounded-xl flex flex-col shadow-2xl overflow-hidden border border-outline-variant/60 bg-surface-container/95 backdrop-blur-sm">
          <!-- Widget Header -->
          <div class="px-4 py-3 border-b border-outline-variant/60 flex items-center justify-between bg-surface-container-high/40">
            <div class="flex items-center gap-2.5">
              <span class="material-symbols-outlined text-primary text-[20px]">grid_view</span>
              <h3 class="font-bold text-sm text-on-surface tracking-wide">{{ t('dashboard.nmsModules') }}</h3>
            </div>
            <div class="flex items-center gap-3">
              <span class="px-2 py-0.5 rounded text-[10px] font-bold font-mono tracking-wider bg-primary/15 text-primary border border-primary/30 uppercase">
                {{ t('dashboard.systemBadge') }}
              </span>
              <button
                type="button"
                class="text-on-surface-variant hover:text-primary transition-colors flex items-center justify-center p-1 rounded hover:bg-surface-variant/40 cursor-pointer"
                @click="handleRefresh"
                title="Refresh"
              >
                <span class="material-symbols-outlined text-[18px]">sync</span>
              </button>
            </div>
          </div>

          <!-- Widget Body -->
          <div class="p-4 flex flex-col gap-3">
            <!-- Status Summary row -->
            <div class="flex items-center justify-between px-3 py-2.5 rounded-lg bg-surface-container-lowest/90 border border-outline-variant/40">
              <div class="flex items-center gap-2.5">
                <span class="material-symbols-outlined text-primary text-[18px]">developer_board</span>
                <span class="font-body-base text-xs font-medium text-on-surface">{{ t('dashboard.moduleSummary') }}</span>
              </div>
              <span class="px-2 py-0.5 rounded text-[10px] font-bold font-mono tracking-wider bg-primary/15 text-primary border border-primary/30">
                {{ modulesStore.activeCount }} / {{ modulesStore.totalCount }} Loaded
              </span>
            </div>

            <!-- Modules list or Empty State -->
            <div v-if="modulesStore.modules.length > 0" class="flex flex-col gap-2 my-1">
              <div
                v-for="mod in modulesStore.modules"
                :key="mod.id"
                class="flex items-center justify-between p-2.5 rounded-lg bg-surface-container-low border border-outline-variant/30 hover:border-primary/50 transition-all cursor-pointer"
                @click="router.push('/modules')"
              >
                <div class="flex items-center gap-3">
                  <span class="material-symbols-outlined text-primary text-[18px]">extension</span>
                  <div>
                    <p class="text-xs font-bold text-on-surface">{{ mod.name }}</p>
                    <p class="text-[10px] text-on-surface-variant font-mono">v{{ mod.version }}</p>
                  </div>
                </div>
                <span
                  class="px-2 py-0.5 rounded text-[10px] font-bold uppercase font-mono"
                  :class="mod.is_active ? 'bg-tertiary/15 text-tertiary border border-tertiary/30' : 'bg-surface-variant text-on-surface-variant border border-outline-variant/40'"
                >
                  {{ mod.is_active ? 'Active' : 'Disabled' }}
                </span>
              </div>
            </div>

            <!-- Empty State -->
            <div v-else class="flex items-center justify-center py-16">
              <p class="text-on-surface-variant/70 text-sm font-medium">{{ t('dashboard.noModules') }}</p>
            </div>
          </div>

          <!-- Widget Footer -->
          <div class="px-4 py-2.5 border-t border-outline-variant/60 flex items-center justify-between bg-surface-container-low/40">
            <span class="text-[11px] font-mono text-outline">Updated: {{ lastUpdated }}</span>
            <button
              type="button"
              class="flex items-center gap-1.5 text-xs font-semibold text-primary hover:text-primary/80 transition-colors cursor-pointer"
              @click="router.push('/modules')"
            >
              {{ t('dashboard.manage') }}
              <span class="material-symbols-outlined text-[16px]">settings</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </main>
</template>
