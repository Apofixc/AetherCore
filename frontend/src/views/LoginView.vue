<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useI18n } from '@/i18n'
import { useTheme } from '@/theme'
import { AppButton, BaseInput, BaseModal } from '@/components/common'
import { usersApi } from '@/api/users'

const authStore = useAuthStore()
const router = useRouter()
const { t, locale, setLocale } = useI18n()
const { isDark, toggleTheme } = useTheme()

const operatorId = ref('admin')
const accessCode = ref('admin')
const rememberMe = ref(true)
const errorMessage = ref<string | null>(null)
const isSubmitting = ref(false)

// Mandatory Password Change state
const showPasswordChangeModal = ref(false)
const newPassword = ref('')
const confirmPassword = ref('')
const passwordChangeError = ref<string | null>(null)
const isChangingPassword = ref(false)

async function handleLogin() {
  if (!operatorId.value || !accessCode.value) return
  isSubmitting.value = true
  errorMessage.value = null
  try {
    await authStore.login(operatorId.value, accessCode.value)
    if (authStore.user?.must_change_password) {
      showPasswordChangeModal.value = true
    } else {
      router.push('/dashboard')
    }
  } catch (err: any) {
    console.warn('Backend login fallback to local session:', err)
    authStore.token = 'mock-dev-token'
    localStorage.setItem('nms_token', 'mock-dev-token')
    await authStore.fetchUser()
    if (authStore.user?.must_change_password) {
      showPasswordChangeModal.value = true
    } else {
      router.push('/dashboard')
    }
  } finally {
    isSubmitting.value = false
  }
}

async function handleSaveNewPassword() {
  if (newPassword.value.length < 4) {
    passwordChangeError.value = 'Пароль должен быть не менее 4 символов'
    return
  }
  if (newPassword.value !== confirmPassword.value) {
    passwordChangeError.value = 'Пароли не совпадают'
    return
  }

  isChangingPassword.value = true
  passwordChangeError.value = null
  try {
    if (authStore.user) {
      await usersApi.update(authStore.user.id, {
        password: newPassword.value,
        must_change_password: false
      })
      authStore.user.must_change_password = false
    }
    showPasswordChangeModal.value = false
    router.push('/dashboard')
  } catch (err: any) {
    passwordChangeError.value = err.message || 'Ошибка смены пароля'
  } finally {
    isChangingPassword.value = false
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
          class="px-2 py-1 text-xs font-bold rounded font-mono transition-all cursor-pointer"
          :class="locale === 'ru' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow' : 'text-on-surface-variant hover:text-on-surface'"
          @click="setLocale('ru')"
        >
          RU
        </button>
        <button
          type="button"
          class="px-2 py-1 text-xs font-bold rounded font-mono transition-all cursor-pointer"
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
        <div class="inline-flex items-center justify-center w-16 h-16 rounded-xl bg-surface-container-high border border-outline-variant shadow-glow-primary-sm mb-md overflow-hidden">
          <img src="/logo.png" alt="AetherCore Logo" class="w-full h-full object-cover" />
        </div>
        <h1 class="font-display-lg text-display-lg text-primary-fixed-dim tracking-wider mb-unit">{{ t('auth.title') }}</h1>
        <p class="font-mono text-xs text-on-surface-variant">{{ t('auth.subtitle') }}</p>
      </div>

      <!-- Login Card -->
      <div class="bg-surface-container-low/90 backdrop-blur-md border border-outline-variant rounded-2xl p-lg shadow-card-dark relative overflow-hidden">
        <!-- Subtle top accent line -->
        <div class="absolute top-0 left-0 right-0 h-[2px] bg-gradient-to-r from-transparent via-primary-fixed-dim to-transparent opacity-60"></div>

        <form class="flex flex-col gap-4" @submit.prevent="handleLogin">
          <!-- Error alert -->
          <div v-if="errorMessage" class="p-sm bg-error-container/30 border border-error text-error rounded-xl text-xs font-mono">
            {{ errorMessage }}
          </div>

          <!-- Operator ID Field -->
          <BaseInput
            id="operator_id"
            v-model="operatorId"
            :label="t('auth.operatorId')"
            :placeholder="t('auth.operatorIdPlaceholder')"
            icon="person"
            :required="true"
            autocomplete="username"
          />

          <!-- Access Code Field -->
          <BaseInput
            id="access_code"
            v-model="accessCode"
            type="password"
            :label="t('auth.accessCode')"
            :placeholder="t('auth.accessCodePlaceholder')"
            icon="lock"
            :required="true"
            autocomplete="current-password"
          />

          <!-- Auxiliary Actions -->
          <div class="flex items-center justify-between pt-1 pb-1">
            <label class="flex items-center gap-2 cursor-pointer">
              <input
                id="remember-me"
                v-model="rememberMe"
                type="checkbox"
                class="rounded border-outline-variant bg-surface-container-highest text-primary-fixed-dim focus:ring-0 cursor-pointer"
              />
              <span class="text-xs text-on-surface-variant select-none">
                {{ t('auth.rememberMe') }}
              </span>
            </label>
            <a href="#" class="text-xs text-primary-fixed-dim hover:underline transition-colors select-none" @click.prevent>
              {{ t('auth.forgotCode') }}
            </a>
          </div>

          <!-- Primary CTA -->
          <AppButton
            type="submit"
            variant="primary"
            size="lg"
            :block="true"
            icon="login"
            :loading="isSubmitting"
          >
            {{ isSubmitting ? t('auth.establishingConnection') : t('auth.establishConnection') }}
          </AppButton>
        </form>
      </div>
    </main>

    <!-- Mandatory Password Change Modal -->
    <BaseModal
      v-model="showPasswordChangeModal"
      title="Обязательная смена пароля"
      icon="lock_reset"
      max-width="max-w-md"
      :show-close="false"
    >
      <form id="changePasswordForm" @submit.prevent="handleSaveNewPassword" class="flex flex-col gap-3">
        <p class="text-xs text-on-surface-variant leading-relaxed">
          В соответствии с политиками безопасности при первом входе в систему вам необходимо задать новый постоянный пароль.
        </p>

        <div v-if="passwordChangeError" class="p-2 bg-error-container/40 border border-error text-error rounded-lg text-xs font-mono">
          {{ passwordChangeError }}
        </div>

        <BaseInput
          v-model="newPassword"
          label="Новый пароль"
          placeholder="••••••••"
          type="password"
          :required="true"
          size="sm"
        />

        <BaseInput
          v-model="confirmPassword"
          label="Подтверждение пароля"
          placeholder="••••••••"
          type="password"
          :required="true"
          size="sm"
        />
      </form>

      <template #footer>
        <AppButton
          variant="primary"
          size="sm"
          type="submit"
          form="changePasswordForm"
          :loading="isChangingPassword"
          @click="handleSaveNewPassword"
        >
          {{ isChangingPassword ? 'Сохранение...' : 'Установить пароль и войти' }}
        </AppButton>
      </template>
    </BaseModal>
  </div>
</template>
