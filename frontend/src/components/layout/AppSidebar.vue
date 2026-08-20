<script setup lang="ts">
import { ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from '@/i18n'

const props = defineProps<{
  collapsed?: boolean
}>()

const emit = defineEmits<{
  (e: 'toggle'): void
}>()

const { t } = useI18n()
const router = useRouter()
const route = useRoute()

const dataProcessorOpen = ref(true)
const fileExplorerOpen = ref(true)
const activeSubItem = ref('overview')

function isCurrent(path: string) {
  return route.path === path
}

function isSettingsActive() {
  return route.path === '/profile' || route.path.startsWith('/settings') || route.path === '/users'
}

function isSubActive(item: string) {
  return route.path === '/modules' && (route.query.tab === item || (!route.query.tab && activeSubItem.value === item))
}

function selectSubItem(item: string) {
  activeSubItem.value = item
  router.push({ path: '/modules', query: { tab: item } })
}
</script>

<template>
  <nav
    id="sidebar"
    class="sidebar-panel w-sidebar-width h-screen fixed left-0 top-0 flex flex-col py-6 px-4 z-50 transition-all duration-300 ease-in-out select-none"
    :class="{ '-translate-x-full': collapsed }"
  >
    <!-- Brand Header -->
    <div
      class="mb-6 flex items-center gap-3 cursor-pointer group"
      @click="router.push('/dashboard')"
    >
      <!-- App Logo Box -->
      <div
        class="sidebar-logo-box w-10 h-10 rounded-xl flex items-center justify-center shrink-0 overflow-hidden group-hover:border-primary/60 transition-all"
      >
        <img alt="AetherCore Logo" class="w-full h-full object-cover" src="/logo.png" />
      </div>
      <!-- App Title & Version -->
      <div class="flex flex-col">
        <span class="font-bold text-[19px] text-primary tracking-tight leading-tight group-hover:text-primary-fixed-dim transition-colors font-display-lg">
          AetherCore
        </span>
        <span class="font-mono text-[11px] text-on-surface-variant tracking-wider mt-0.5 font-body-mono">
          Version 1.0.4
        </span>
      </div>
    </div>

    <!-- Main Navigation Content -->
    <div class="flex-1 flex flex-col overflow-y-auto pr-1 -mr-1 space-y-6">
      <!-- CORE MODULES -->
      <div>
        <h3 class="text-[11px] font-bold font-mono tracking-widest text-on-surface-variant uppercase mb-2.5 px-1">
          CORE MODULES
        </h3>
        <router-link
          to="/dashboard"
          class="h-12 px-5 flex items-center gap-4 rounded-xl transition-all duration-200 ease-in-out shrink-0 cursor-pointer group relative overflow-hidden text-[15px]"
          :class="isCurrent('/dashboard')
            ? 'sidebar-item-active'
            : 'border border-transparent text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/40 font-medium'"
        >
          <span
            class="material-symbols-outlined text-[22px] transition-transform group-hover:scale-105"
            :class="isCurrent('/dashboard') ? 'text-primary group-hover:brightness-125' : 'text-on-surface-variant group-hover:text-on-surface'"
            :style="isCurrent('/dashboard') ? 'font-variation-settings: &quot;FILL&quot; 1;' : ''"
          >
            grid_view
          </span>
          <span class="tracking-normal font-sans">Dashboard</span>
        </router-link>
      </div>

      <!-- DYNAMIC MODULES -->
      <div class="flex flex-col">
        <h3 class="text-[11px] font-bold font-mono tracking-widest text-on-surface-variant uppercase mb-2.5 px-1">
          DYNAMIC MODULES
        </h3>

        <!-- Data Processor Accordion -->
        <div class="flex flex-col mb-1.5">
          <button
            type="button"
            class="sidebar-accordion-btn h-10 px-3 flex items-center justify-between w-full cursor-pointer text-sm font-medium group"
            @click="dataProcessorOpen = !dataProcessorOpen"
          >
            <div class="flex items-center gap-3">
              <span class="sidebar-accordion-icon material-symbols-outlined text-[20px]">
                monitoring
              </span>
              <span class="transition-colors">
                Data Processor
              </span>
            </div>
            <span
              class="sidebar-accordion-chevron material-symbols-outlined text-[18px]"
              :class="{ 'rotate-180': dataProcessorOpen }"
            >
              expand_more
            </span>
          </button>
          
          <!-- Animated Dropdown -->
          <div
            class="grid transition-all duration-250 ease-in-out overflow-hidden"
            :style="{
              gridTemplateRows: dataProcessorOpen ? '1fr' : '0fr',
              opacity: dataProcessorOpen ? 1 : 0
            }"
          >
            <div class="min-h-0 ml-[23px] pl-3.5 border-l border-outline-variant flex flex-col gap-1 py-1.5 mt-0.5">
              <button
                type="button"
                class="text-xs text-left w-full transition-all rounded-md px-2.5 py-1.5 flex items-center justify-between group cursor-pointer"
                :class="isSubActive('overview')
                  ? 'sidebar-sub-item-active'
                  : 'text-on-surface-variant hover:text-primary hover:bg-primary/5 hover:translate-x-1 border border-transparent'"
                @click="selectSubItem('overview')"
              >
                <span>Overview</span>
                <span v-if="isSubActive('overview')" class="sidebar-sub-item-dot w-1.5 h-1.5 rounded-full"></span>
              </button>
              <button
                type="button"
                class="text-xs text-left w-full transition-all rounded-md px-2.5 py-1.5 flex items-center justify-between group cursor-pointer"
                :class="isSubActive('transform')
                  ? 'sidebar-sub-item-active'
                  : 'text-on-surface-variant hover:text-primary hover:bg-primary/5 hover:translate-x-1 border border-transparent'"
                @click="selectSubItem('transform')"
              >
                <span>Transform</span>
                <span v-if="isSubActive('transform')" class="sidebar-sub-item-dot w-1.5 h-1.5 rounded-full"></span>
              </button>
              <button
                type="button"
                class="text-xs text-left w-full transition-all rounded-md px-2.5 py-1.5 flex items-center justify-between group cursor-pointer"
                :class="isSubActive('export')
                  ? 'sidebar-sub-item-active'
                  : 'text-on-surface-variant hover:text-primary hover:bg-primary/5 hover:translate-x-1 border border-transparent'"
                @click="selectSubItem('export')"
              >
                <span>Export</span>
                <span v-if="isSubActive('export')" class="sidebar-sub-item-dot w-1.5 h-1.5 rounded-full"></span>
              </button>
            </div>
          </div>
        </div>

        <!-- File Explorer Accordion -->
        <div class="flex flex-col mb-3">
          <button
            type="button"
            class="sidebar-accordion-btn h-10 px-3 flex items-center justify-between w-full cursor-pointer text-sm font-medium group"
            @click="fileExplorerOpen = !fileExplorerOpen"
          >
            <div class="flex items-center gap-3">
              <span class="sidebar-accordion-icon material-symbols-outlined text-[20px]">
                folder
              </span>
              <span class="transition-colors">
                File Explorer
              </span>
            </div>
            <span
              class="sidebar-accordion-chevron material-symbols-outlined text-[18px]"
              :class="{ 'rotate-180': fileExplorerOpen }"
            >
              expand_more
            </span>
          </button>
          
          <!-- Animated Dropdown -->
          <div
            class="grid transition-all duration-250 ease-in-out overflow-hidden"
            :style="{
              gridTemplateRows: fileExplorerOpen ? '1fr' : '0fr',
              opacity: fileExplorerOpen ? 1 : 0
            }"
          >
            <div class="min-h-0 ml-[23px] pl-3.5 border-l border-outline-variant flex flex-col gap-1 py-1.5 mt-0.5">
              <button
                type="button"
                class="text-xs text-left w-full transition-all rounded-md px-2.5 py-1.5 flex items-center justify-between group cursor-pointer"
                :class="isSubActive('local-storage')
                  ? 'sidebar-sub-item-active'
                  : 'text-on-surface-variant hover:text-primary hover:bg-primary/5 hover:translate-x-1 border border-transparent'"
                @click="selectSubItem('local-storage')"
              >
                <span>Local Storage</span>
                <span v-if="isSubActive('local-storage')" class="sidebar-sub-item-dot w-1.5 h-1.5 rounded-full"></span>
              </button>
              <button
                type="button"
                class="text-xs text-left w-full transition-all rounded-md px-2.5 py-1.5 flex items-center justify-between group cursor-pointer"
                :class="isSubActive('vault')
                  ? 'sidebar-sub-item-active'
                  : 'text-on-surface-variant hover:text-primary hover:bg-primary/5 hover:translate-x-1 border border-transparent'"
                @click="selectSubItem('vault')"
              >
                <span>Vault</span>
                <span v-if="isSubActive('vault')" class="sidebar-sub-item-dot w-1.5 h-1.5 rounded-full"></span>
              </button>
            </div>
          </div>
        </div>

        <!-- Add Module Button -->
        <button
          type="button"
          class="sidebar-add-btn h-12 w-full flex items-center justify-start gap-4 px-5 mt-2 font-medium text-[15px] rounded-xl transition-all cursor-pointer active:scale-[0.99] group"
          @click="router.push('/modules')"
        >
          <span class="material-symbols-outlined text-[22px] font-light">add</span>
          <span class="tracking-normal font-sans">Add Module</span>
        </button>
      </div>
    </div>

    <!-- Footer Status & Settings -->
    <div class="mt-auto flex flex-col gap-3 border-t border-outline-variant pt-4">
      <!-- Health Status Badge -->
      <div class="sidebar-health-badge rounded-lg px-3.5 py-2.5 flex items-center gap-2.5">
        <span class="sidebar-health-indicator w-2 h-2 rounded-full animate-pulse shrink-0"></span>
        <span class="font-mono text-[11px] font-bold text-tertiary tracking-wider uppercase">
          NMS HEALTH: OPTIMAL
        </span>
      </div>

      <!-- Settings Link -->
      <router-link
        to="/profile"
        class="h-12 px-5 flex items-center gap-4 rounded-xl transition-all duration-200 ease-in-out font-medium cursor-pointer group relative overflow-hidden text-[15px]"
        :class="isSettingsActive()
          ? 'sidebar-item-active'
          : 'border border-transparent text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/40'"
      >
        <span
          class="material-symbols-outlined text-[22px] transition-transform group-hover:scale-105"
          :class="isSettingsActive() ? 'text-primary group-hover:brightness-125' : 'text-on-surface-variant group-hover:text-on-surface'"
          :style="isSettingsActive() ? 'font-variation-settings: &quot;FILL&quot; 1;' : ''"
        >
          settings
        </span>
        <span :class="isSettingsActive() ? 'font-bold' : 'text-on-surface'">Settings</span>
      </router-link>
    </div>
  </nav>
</template>
