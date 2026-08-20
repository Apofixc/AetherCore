<script setup lang="ts">
import { useI18n } from '@/i18n'

const { t } = useI18n()
</script>

<template>
  <footer
    id="footer"
    class="fixed bottom-0 ml-sidebar-width w-[calc(100vw-312px)] h-8 bg-surface-container-lowest border-t border-outline-variant flex items-center justify-between px-lg z-50 font-body-mono text-[10px] tracking-wider transition-all duration-300 select-none"
  >
    <div class="flex items-center gap-md">
      <span class="text-primary-fixed-dim">{{ t('common.builtWith') }}</span>
      <span class="text-outline-variant">|</span>
      <div class="flex items-center gap-xs text-tertiary-fixed-dim">
        <span class="material-symbols-outlined text-[14px]">check_circle</span>
        <span>{{ t('common.systemOk') }}</span>
      </div>
    </div>
    <div class="flex items-center gap-lg text-on-surface-variant">
      <a class="hover:text-primary-fixed-dim transition-colors" href="/docs" target="_blank">{{ t('common.apiDocs') }}</a>
      <a class="hover:text-primary-fixed-dim transition-colors" href="https://github.com" target="_blank">{{ t('common.github') }}</a>
      <a class="hover:text-primary-fixed-dim transition-colors" href="https://discord.com" target="_blank">{{ t('common.discord') }}</a>
    </div>
  </footer>
</template>
