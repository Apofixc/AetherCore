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

const dataProcessorOpen = ref(false)
const fileExplorerOpen = ref(false)
const activeSubItem = ref('overview')

function isCurrent(path: string) {
  return route.path === path
}

function isSettingsActive() {
  return route.path === '/profile' || route.path.startsWith('/settings') || route.path === '/users'
}

function selectSubItem(item: string) {
  activeSubItem.value = item
  router.push({ path: '/modules', query: { tab: item } })
}
</script>

<template>
  <nav
    id="sidebar"
    class="bg-surface-container-lowest text-primary font-body-base text-body-base w-sidebar-width h-screen fixed left-0 top-0 border-r border-outline-variant flex flex-col py-lg px-md z-50 transition-transform duration-300 select-none"
    :class="{ '-translate-x-full': collapsed }"
  >
    <!-- Header / Brand -->
    <div
      class="mb-xl flex items-center gap-sm cursor-pointer group"
      @click="router.push('/dashboard')"
    >
      <div class="w-10 h-10 rounded-lg overflow-hidden flex items-center justify-center shrink-0 border border-outline-variant group-hover:border-primary-fixed-dim/60 transition-colors">
        <img
          alt="AetherCore Logo"
          class="w-full h-full object-cover"
          src="/logo.png"
        />
      </div>
      <div>
        <h1 class="font-display-lg text-display-lg text-primary-fixed-dim tracking-wider group-hover:text-primary transition-colors">AetherCore</h1>
        <p class="font-body-mono text-body-mono text-on-surface-variant">{{ t('common.version') }}</p>
      </div>
    </div>

    <!-- Main Navigation Content -->
    <div class="flex-1 flex flex-col gap-xs overflow-y-auto pr-1">
      <!-- CORE MODULES -->
      <div class="mb-md">
        <h3 class="text-label-caps font-label-caps text-on-surface-variant uppercase tracking-wider px-md mb-sm">
          {{ t('nav.coreModules') }}
        </h3>
        <router-link
          to="/dashboard"
          class="flex items-center gap-md px-md py-sm rounded-lg transition-all duration-200 ease-in-out shrink-0 cursor-pointer"
          :class="isCurrent('/dashboard') || isCurrent('/')
            ? 'bg-gradient-to-r from-primary-fixed-dim/20 to-transparent text-primary-fixed-dim font-bold border-l-2 border-primary-fixed-dim shadow-[inset_0_0_10px_rgba(115,212,232,0.15)] hover:from-primary-fixed-dim/30 hover:to-transparent'
            : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50'"
        >
          <span
            class="material-symbols-outlined"
            :class="(isCurrent('/dashboard') || isCurrent('/')) ? 'icon-fill' : ''"
          >
            dashboard
          </span>
          {{ t('nav.dashboard') }}
        </router-link>
      </div>

      <!-- DYNAMIC MODULES -->
      <div class="flex flex-col gap-xs">
        <h3 class="text-label-caps font-label-caps text-on-surface-variant uppercase tracking-wider px-md mb-sm mt-md">
          {{ t('nav.dynamicModules') }}
        </h3>

        <!-- Data Processor Accordion -->
        <div class="flex flex-col">
          <button
            type="button"
            class="flex items-center justify-between w-full px-md py-sm rounded-lg text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all duration-200 ease-in-out cursor-pointer"
            @click="dataProcessorOpen = !dataProcessorOpen"
          >
            <div class="flex items-center gap-md">
              <span class="material-symbols-outlined">analytics</span>
              {{ t('nav.dataProcessor') }}
            </div>
            <span
              class="material-symbols-outlined text-sm transition-transform duration-300"
              :class="{ 'rotate-180': dataProcessorOpen }"
            >
              expand_more
            </span>
          </button>
          <div
            v-show="dataProcessorOpen"
            class="ml-xl pl-md border-l border-outline-variant/50 flex flex-col gap-xs mt-xs transition-all duration-300 overflow-hidden"
          >
            <a
              href="#"
              class="py-xs px-2 text-sm rounded transition-all duration-200"
              :class="activeSubItem === 'overview' && route.path === '/modules' ? 'text-primary-fixed-dim bg-primary-fixed-dim/10 font-semibold' : 'text-on-surface-variant hover:text-primary-fixed-dim hover:bg-primary-fixed-dim/10'"
              @click.prevent="selectSubItem('overview')"
            >
              {{ t('nav.overview') }}
            </a>
            <a
              href="#"
              class="py-xs px-2 text-sm rounded transition-all duration-200"
              :class="activeSubItem === 'transform' && route.path === '/modules' ? 'text-primary-fixed-dim bg-primary-fixed-dim/10 font-semibold' : 'text-on-surface-variant hover:text-primary-fixed-dim hover:bg-primary-fixed-dim/10'"
              @click.prevent="selectSubItem('transform')"
            >
              {{ t('nav.transform') }}
            </a>
            <a
              href="#"
              class="py-xs px-2 text-sm rounded transition-all duration-200"
              :class="activeSubItem === 'export' && route.path === '/modules' ? 'text-primary-fixed-dim bg-primary-fixed-dim/10 font-semibold' : 'text-on-surface-variant hover:text-primary-fixed-dim hover:bg-primary-fixed-dim/10'"
              @click.prevent="selectSubItem('export')"
            >
              {{ t('nav.export') }}
            </a>
          </div>
        </div>

        <!-- File Explorer Accordion -->
        <div class="flex flex-col mt-sm">
          <button
            type="button"
            class="flex items-center justify-between w-full px-md py-sm rounded-lg text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all duration-200 ease-in-out cursor-pointer"
            @click="fileExplorerOpen = !fileExplorerOpen"
          >
            <div class="flex items-center gap-md">
              <span class="material-symbols-outlined">folder</span>
              {{ t('nav.fileExplorer') }}
            </div>
            <span
              class="material-symbols-outlined text-sm transition-transform duration-300"
              :class="{ 'rotate-180': fileExplorerOpen }"
            >
              expand_more
            </span>
          </button>
          <div
            v-show="fileExplorerOpen"
            class="ml-xl pl-md border-l border-outline-variant/50 flex flex-col gap-xs mt-xs transition-all duration-300 overflow-hidden"
          >
            <a
              href="#"
              class="py-xs px-2 text-sm rounded transition-all duration-200"
              :class="activeSubItem === 'local' && route.path === '/modules' ? 'text-primary-fixed-dim bg-primary-fixed-dim/10 font-semibold' : 'text-on-surface-variant hover:text-primary-fixed-dim hover:bg-primary-fixed-dim/10'"
              @click.prevent="selectSubItem('local')"
            >
              {{ t('nav.localStorage') }}
            </a>
            <a
              href="#"
              class="py-xs px-2 text-sm rounded transition-all duration-200"
              :class="activeSubItem === 'vault' && route.path === '/modules' ? 'text-primary-fixed-dim bg-primary-fixed-dim/10 font-semibold' : 'text-on-surface-variant hover:text-primary-fixed-dim hover:bg-primary-fixed-dim/10'"
              @click.prevent="selectSubItem('vault')"
            >
              {{ t('nav.vault') }}
            </a>
          </div>
        </div>

        <!-- Add Module Button -->
        <button
          type="button"
          class="flex items-center gap-md px-md py-sm mt-lg text-primary-fixed-dim hover:text-primary-fixed-dim/90 transition-all duration-200 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 shadow-glow-primary-sm hover:bg-primary-fixed-dim/20 hover:shadow-glow-primary-md cursor-pointer active:scale-95"
          @click="router.push('/modules')"
        >
          <span class="material-symbols-outlined">add</span>
          {{ t('nav.addModule') }}
        </button>
      </div>
    </div>

    <!-- Footer Tabs & CTA -->
    <div class="mt-auto flex flex-col gap-sm border-t border-outline-variant pt-md">
      <router-link
        to="/settings/access-identity"
        class="flex items-center gap-md px-md py-sm rounded-lg transition-all duration-200 ease-in-out cursor-pointer"
        :class="isSettingsActive()
          ? 'bg-gradient-to-r from-primary-fixed-dim/20 to-transparent border-l-2 border-primary-fixed-dim text-primary-fixed-dim font-bold shadow-[inset_0_0_10px_rgba(115,212,232,0.15)] hover:from-primary-fixed-dim/30 hover:to-transparent'
          : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50'"
      >
        <span
          class="material-symbols-outlined"
          :class="isSettingsActive() ? 'icon-fill' : ''"
        >
          settings
        </span>
        {{ t('nav.settings') }}
      </router-link>
    </div>
  </nav>
</template>
