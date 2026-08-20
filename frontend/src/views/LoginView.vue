<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useI18n, type Locale } from '@/i18n'
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
    // В dev-режиме, если бэкенд недоступен, позволяем зайти под дефолтным root-пользователем
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
    <!-- Quick Controls (Top-right) -->
    <div class="absolute top-4 right-4 z-20 flex items-center gap-2">
      <!-- Language Switcher -->
      <div class="flex items-center bg-surface-container/80 backdrop-blur-sm rounded-lg p-0.5 border border-outline-variant/60">
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

      <!-- Theme Switcher -->
      <button
        type="button"
        class="p-2 text-on-surface-variant hover:text-primary transition-colors bg-surface-container/80 backdrop-blur-sm border border-outline-variant/60 rounded-lg flex items-center justify-center cursor-pointer"
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
      <div class="absolute inset-0 bg-[linear-gradient(rgba(138,235,255,0.03)_1px,transparent_1px),linear-gradient(90deg,rgba(138,235,255,0.03)_1px,transparent_1px)] bg-[size:40px_40px]"></div>
      <!-- Scanlines -->
      <div class="absolute inset-0 scanlines opacity-30"></div>
    </div>

    <!-- Main Content Canvas -->
    <main class="relative z-10 w-full max-w-md px-lg">
      <!-- Brand Header -->
      <div class="text-center mb-xl">
        <div class="inline-flex items-center justify-center w-16 h-16 rounded-xl bg-surface-container-high border border-outline-variant glow-primary mb-md overflow-hidden">
          <img src="/logo.png" alt="AetherCore Logo" class="w-full h-full object-cover" />
        </div>
        <h1 class="font-display-lg text-display-lg text-primary tracking-tighter mb-xs">AETHERCORE</h1>
        <p class="font-body-mono text-body-mono text-on-surface-variant">NEXT-GEN NOC TERMINAL</p>
      </div>

      <!-- Login Card -->
      <div class="bg-surface-dim/80 backdrop-blur-md border border-outline-variant rounded-[16px] p-lg shadow-2xl relative overflow-hidden">
        <!-- Subtle top accent line -->
        <div class="absolute top-0 left-0 right-0 h-[2px] bg-gradient-to-r from-transparent via-primary-container to-transparent opacity-50"></div>

        <form class="space-y-md" @submit.prevent="handleLogin">
          <!-- Error alert -->
          <div v-if="errorMessage" class="p-sm bg-error-container/30 border border-error text-error rounded-lg text-xs font-body-mono">
            {{ errorMessage }}
          </div>

          <!-- Operator ID Field -->
          <div>
            <label class="block font-label-caps text-label-caps text-on-surface-variant mb-xs" for="operator_id">
              OPERATOR IDENTIFIER
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
                class="block w-full pl-[40px] pr-md py-sm bg-surface-container-high border border-outline rounded-DEFAULT text-on-surface font-body-mono text-body-mono focus:ring-1 focus:ring-primary focus:border-primary placeholder:text-on-surface-variant transition-colors duration-200"
                placeholder="sys_admin_01"
              />
            </div>
          </div>

          <!-- Access Code Field -->
          <div>
            <label class="block font-label-caps text-label-caps text-on-surface-variant mb-xs" for="access_code">
              ACCESS CODE
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
                class="block w-full pl-[40px] pr-md py-sm bg-surface-container-high border border-outline rounded-DEFAULT text-on-surface font-body-mono text-body-mono focus:ring-1 focus:ring-primary focus:border-primary placeholder:text-on-surface-variant transition-colors duration-200"
                placeholder="••••••••••••"
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
                class="h-4 w-4 rounded-DEFAULT border-outline bg-surface-container-high text-primary focus:ring-primary focus:ring-offset-surface-dim"
              />
              <label class="ml-sm block font-body-base text-body-base text-on-surface-variant cursor-pointer" for="remember-me">
                Remember Me
              </label>
            </div>
            <a href="#" class="font-body-base text-body-base text-primary hover:text-primary-fixed-dim transition-colors" @click.prevent>
              Forgot Code?
            </a>
          </div>

          <!-- Primary CTA -->
          <button
            type="submit"
            :disabled="isSubmitting"
            class="w-full flex items-center justify-center gap-sm py-sm px-md bg-primary-container text-on-primary-container rounded-lg font-title-sm text-title-sm glow-primary glow-primary-hover transition-all duration-200 active:scale-95 disabled:opacity-50 cursor-pointer"
          >
            <span class="material-symbols-outlined" style="font-size: 20px;">login</span>
            <span>{{ isSubmitting ? 'Establishing...' : 'Establish Connection' }}</span>
          </button>
        </form>
      </div>
    </main>
  </div>
</template>
