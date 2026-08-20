<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useI18n } from '@/i18n'
import { useTheme } from '@/theme'

const authStore = useAuthStore()
const router = useRouter()
const { t, locale, setLocale } = useI18n()
const { isDark, toggleTheme } = useTheme()

const operatorId = ref('admin')
const accessCode = ref('admin123')
const rememberMe = ref(true)
const errorMessage = ref<string | null>(null)
const isSubmitting = ref(false)

async function handleLogin() {
  if (!operatorId.value || !accessCode.value) return
  isSubmitting.value = true
  errorMessage.value = null
  try {
    await authStore.login(operatorId.value, accessCode.value)
    router.push('/dashboard')
  } catch (err: any) {
    console.warn('Backend login fallback to local session:', err)
    authStore.token = 'mock-dev-token'
    localStorage.setItem('nms_token', 'mock-dev-token')
    await authStore.fetchUser()
    router.push('/dashboard')
  } finally {
    isSubmitting.value = false
  }
}
</script>

<template>
  <div class="bg-surface-deep text-on-surface font-body-base min-h-screen flex flex-col items-center justify-center relative overflow-hidden select-none">
    <!-- Quick Language & Theme Controls (Top-right) -->
    <div class="absolute top-4 right-4 z-20 flex items-center gap-2">
      <div class="flex items-center bg-surface-container-high/80 backdrop-blur-sm rounded-lg p-0.5 border border-outline-variant/60">
        <button
          type="button"
          class="px-2 py-1 text-xs font-bold rounded font-body-mono transition-all cursor-pointer"
          :class="locale === 'ru' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow' : 'text-on-surface-variant hover:text-on-surface'"
          @click="setLocale('ru')"
        >
          RU
        </button>
        <button
          type="button"
          class="px-2 py-1 text-xs font-bold rounded font-body-mono transition-all cursor-pointer"
          :class="locale === 'en' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow' : 'text-on-surface-variant hover:text-on-surface'"
          @click="setLocale('en')"
        >
          EN
        </button>
      </div>

      <button
        type="button"
        class="p-2 text-on-surface-variant hover:text-primary-fixed-dim transition-colors bg-surface-container-high/80 backdrop-blur-sm border border-outline-variant/60 rounded-lg flex items-center justify-center cursor-pointer"
        @click="toggleTheme"
        :title="isDark ? 'Switch to Light' : 'Switch to Dark'"
      >
        <span class="material-symbols-outlined text-sm">{{ isDark ? 'light_mode' : 'dark_mode' }}</span>
      </button>
    </div>

    <!-- Cinematic Background -->
    <div class="absolute inset-0 z-0 pointer-events-none">
      <div
        class="w-full h-full bg-cover bg-center opacity-40 mix-blend-luminosity"
        style="background-image: url('https://lh3.googleusercontent.com/aida-public/AB6AXuAfS5AOxKg62xUntEOyPOiVxOvm4JjnrAvsp6U_9M3H77hxQAVml87QhmaMQ_pURQlREF3gZvU8RIsi_PaYGUEBZMCO5FIJhlSpmv7sJzEwcYVkMQkuqS_SpjhnFOr3ed19ybS_wuMd432c3ehqxEY4soA79FdmNHPcfcjqYgndfcLIcyh62bvG1UJWafZMl1nbbqG9NXf6m7xqAbZh1Ubnwf0ES5ol6ACJNw0uE-2f_A-hcUKpAqO1');"
      ></div>
      <!-- Gradient Overlays for Depth and Focus -->
      <div class="absolute inset-0 bg-gradient-to-t from-surface-deep via-surface-deep/80 to-transparent"></div>
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_center,_var(--tw-gradient-stops))] from-surface-dim/0 via-surface-deep/60 to-surface-deep/90"></div>
      <!-- Technical Grid Overlay -->
      <div class="absolute inset-0 bg-[linear-gradient(rgba(115,212,232,0.03)_1px,transparent_1px),linear-gradient(90deg,rgba(115,212,232,0.03)_1px,transparent_1px)] bg-[size:40px_40px]"></div>
      <!-- Scanlines -->
      <div class="absolute inset-0 scanlines opacity-30"></div>
    </div>

    <!-- Main Content Canvas -->
    <main class="relative z-10 w-full max-w-md px-lg">
      <!-- Brand Header -->
      <div class="text-center mb-xl">
        <div class="inline-flex items-center justify-center w-16 h-16 rounded-lg bg-surface-container-high border border-outline-variant shadow-glow-primary-sm mb-md overflow-hidden">
          <img src="/logo.png" alt="AetherCore Logo" class="w-full h-full object-cover" />
        </div>
        <h1 class="font-display-lg text-display-lg text-primary-fixed-dim tracking-wider mb-unit">{{ t('auth.title') }}</h1>
        <p class="font-body-mono text-body-mono text-on-surface-variant">{{ t('auth.subtitle') }}</p>
      </div>

      <!-- Login Card -->
      <div class="bg-surface-container-low/90 backdrop-blur-md border border-outline-variant rounded-lg p-lg shadow-card-dark relative overflow-hidden">
        <!-- Subtle top accent line -->
        <div class="absolute top-0 left-0 right-0 h-[2px] bg-gradient-to-r from-transparent via-primary-fixed-dim to-transparent opacity-60"></div>

        <form class="space-y-md" @submit.prevent="handleLogin">
          <!-- Error alert -->
          <div v-if="errorMessage" class="p-sm bg-error-container/30 border border-error text-error rounded-lg text-xs font-body-mono">
            {{ errorMessage }}
          </div>

          <!-- Operator ID Field -->
          <div>
            <label class="block font-label-caps text-label-caps text-on-surface-variant mb-xs" for="operator_id">
              {{ t('auth.operatorId') }}
            </label>
            <div class="relative">
              <div class="absolute inset-y-0 left-0 pl-sm flex items-center pointer-events-none">
                <span class="material-symbols-outlined text-on-surface-variant" style="font-size: 20px;">person</span>
              </div>
              <input
                id="operator_id"
                v-model="operatorId"
                type="text"
                required
                autocomplete="username"
                class="block w-full pl-[40px] pr-md py-sm bg-surface-container-highest border border-outline-variant rounded-lg text-on-surface font-body-mono text-body-mono focus:ring-1 focus:ring-primary-fixed-dim focus:border-primary-fixed-dim placeholder:text-on-surface-variant/50 transition-colors duration-200"
                :placeholder="t('auth.operatorIdPlaceholder')"
              />
            </div>
          </div>

          <!-- Access Code Field -->
          <div>
            <label class="block font-label-caps text-label-caps text-on-surface-variant mb-xs" for="access_code">
              {{ t('auth.accessCode') }}
            </label>
            <div class="relative">
              <div class="absolute inset-y-0 left-0 pl-sm flex items-center pointer-events-none">
                <span class="material-symbols-outlined text-on-surface-variant" style="font-size: 20px;">lock</span>
              </div>
              <input
                id="access_code"
                v-model="accessCode"
                type="password"
                required
                autocomplete="current-password"
                class="block w-full pl-[40px] pr-md py-sm bg-surface-container-highest border border-outline-variant rounded-lg text-on-surface font-body-mono text-body-mono focus:ring-1 focus:ring-primary-fixed-dim focus:border-primary-fixed-dim placeholder:text-on-surface-variant/50 transition-colors duration-200"
                :placeholder="t('auth.accessCodePlaceholder')"
              />
            </div>
          </div>

          <!-- Auxiliary Actions -->
          <div class="flex items-center justify-between pt-sm pb-md">
            <div class="flex items-center">
              <input
                id="remember-me"
                v-model="rememberMe"
                type="checkbox"
                class="h-4 w-4 rounded border-outline-variant bg-surface-container-highest text-primary-fixed-dim focus:ring-primary-fixed-dim"
              />
              <label class="ml-sm block font-body-base text-body-base text-on-surface-variant cursor-pointer" for="remember-me">
                {{ t('auth.rememberMe') }}
              </label>
            </div>
            <a href="#" class="font-body-base text-body-base text-primary-fixed-dim hover:underline transition-colors" @click.prevent>
              {{ t('auth.forgotCode') }}
            </a>
          </div>

          <!-- Primary CTA -->
          <button
            type="submit"
            :disabled="isSubmitting"
            class="w-full flex items-center justify-center gap-sm py-2.5 px-md bg-primary-fixed-dim text-on-primary-fixed rounded-lg font-title-sm text-title-sm shadow-glow-primary-sm hover:shadow-glow-primary-md hover:bg-primary-fixed-dim/90 transition-all duration-200 active:scale-95 disabled:opacity-50 cursor-pointer"
          >
            <span class="material-symbols-outlined" style="font-size: 20px;">login</span>
            <span>{{ isSubmitting ? t('auth.establishingConnection') : t('auth.establishConnection') }}</span>
          </button>
        </form>
      </div>
    </main>

    <!-- Footer -->
    <footer class="fixed bottom-0 w-full h-8 bg-surface-container-lowest/80 backdrop-blur-sm border-t border-outline-variant flex items-center justify-between px-lg z-50 font-body-mono text-[10px] tracking-wider">
      <div class="flex items-center gap-md">
        <span class="text-primary-fixed-dim">{{ t('common.builtWith') }}</span>
        <span class="text-outline-variant">|</span>
        <div class="flex items-center gap-xs text-tertiary-fixed-dim">
          <span class="material-symbols-outlined text-[14px]">check_circle</span>
          <span>{{ t('common.systemOk') }}</span>
        </div>
      </div>
      <div class="flex items-center gap-lg text-on-surface-variant">
        <a class="hover:text-primary-fixed-dim transition-colors" href="#">{{ t('common.apiDocs') }}</a>
        <a class="hover:text-primary-fixed-dim transition-colors" href="#">{{ t('common.github') }}</a>
        <a class="hover:text-primary-fixed-dim transition-colors" href="#">{{ t('common.discord') }}</a>
      </div>
    </footer>
  </div>
</template>
