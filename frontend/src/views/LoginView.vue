<script setup lang="ts">
import { ref, onMounted } from 'vue'
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
const customUsername = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const passwordChangeError = ref<string | null>(null)
const isChangingPassword = ref(false)

onMounted(async () => {
  await authStore.checkAuthConfig()
  if (authStore.authConfig?.web_ui_auth === false) {
    router.push('/dashboard')
  }
})

async function handleLogin() {
  if (!operatorId.value || !accessCode.value) return
  isSubmitting.value = true
  errorMessage.value = null
  try {
    await authStore.login(operatorId.value, accessCode.value)
    const isFirstLogin = !authStore.user?.last_login_at || authStore.user?.must_change_password
    if (isFirstLogin && authStore.user && authStore.user.username !== 'root') {
      customUsername.value = authStore.user.username || ''
      showPasswordChangeModal.value = true
    } else {
      router.push('/dashboard')
    }
  } catch (err: any) {
    console.error('Login failed:', err)
    errorMessage.value = err?.message || t('auth.invalidCredentials')
  } finally {
    isSubmitting.value = false
  }
}

async function handleSaveNewPassword() {
  if (customUsername.value.trim().length < 3) {
    passwordChangeError.value = t('auth.invalidUsernameLength')
    return
  }

  const isMandatoryPassword = Boolean(authStore.user?.must_change_password)
  const hasPasswordInput = newPassword.value.length > 0 || confirmPassword.value.length > 0

  if (isMandatoryPassword || hasPasswordInput) {
    if (newPassword.value.length < 4) {
      passwordChangeError.value = t('auth.passwordTooShort')
      return
    }
    if (newPassword.value !== confirmPassword.value) {
      passwordChangeError.value = t('auth.passwordsDoNotMatch')
      return
    }
  }

  isChangingPassword.value = true
  passwordChangeError.value = null
  try {
    if (authStore.user) {
      const payload: { username: string; password?: string; must_change_password?: boolean } = {
        username: customUsername.value.trim()
      }
      if (hasPasswordInput) {
        payload.password = newPassword.value
        payload.must_change_password = false
      }
      const updated = await usersApi.update(authStore.user.id, payload)
      authStore.user = updated
    }
    showPasswordChangeModal.value = false
    router.push('/dashboard')
  } catch (err: any) {
    passwordChangeError.value = err.message || t('auth.passwordChangeError')
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
        class="w-8 h-8 rounded-lg bg-surface-container-high/80 backdrop-blur-sm border border-outline-variant/60 flex items-center justify-center text-on-surface-variant hover:text-on-surface transition-colors cursor-pointer"
        :title="isDark ? t('auth.themeLight') : t('auth.themeDark')"
        @click="toggleTheme"
      >
        <span class="material-symbols-outlined text-[18px]">
          {{ isDark ? 'light_mode' : 'dark_mode' }}
        </span>
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

    <!-- Main Content Center -->
    <main class="w-full max-w-sm px-4 flex flex-col items-center relative z-10">
      <!-- Title & Branding -->
      <div class="flex flex-col items-center gap-2 mb-6">
        <div class="w-12 h-12 rounded-xl bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shadow-glow">
          <span class="material-symbols-outlined text-2xl">hub</span>
        </div>
        <div class="text-center">
          <h1 class="text-xl font-bold font-mono tracking-widest text-on-surface uppercase">{{ t('auth.title') }}</h1>
          <p class="text-xs text-on-surface-variant font-mono tracking-wider">{{ t('auth.subtitle') }}</p>
        </div>
      </div>

      <!-- Login Form Card -->
      <div class="w-full bg-surface-container-low border border-outline-variant/60 rounded-xl p-6 shadow-card-dark flex flex-col gap-4">
        <!-- Error Alert -->
        <div v-if="errorMessage" class="p-3 bg-error-container/30 border border-error text-error text-xs rounded-lg flex items-center gap-2">
          <span class="material-symbols-outlined text-base shrink-0">error</span>
          <span>{{ errorMessage }}</span>
        </div>

        <form @submit.prevent="handleLogin" class="flex flex-col gap-4">
          <!-- Operator ID Input -->
          <BaseInput
            v-model="operatorId"
            :label="t('auth.operatorId')"
            :placeholder="t('auth.operatorIdPlaceholder')"
            icon="badge"
            :required="true"
            :autofocus="true"
          />

          <!-- Access Code Input -->
          <BaseInput
            v-model="accessCode"
            :label="t('auth.accessCode')"
            :placeholder="t('auth.accessCodePlaceholder')"
            type="password"
            icon="key"
            :required="true"
          />

          <!-- Remember me & Help -->
          <div class="flex items-center justify-between text-xs text-on-surface-variant">
            <label class="flex items-center gap-2 cursor-pointer select-none">
              <input
                v-model="rememberMe"
                type="checkbox"
                class="rounded border-outline-variant bg-surface-container-highest text-primary-fixed-dim focus:ring-0 cursor-pointer"
              />
              <span>{{ t('auth.rememberMe') }}</span>
            </label>
            <a href="#" class="hover:text-primary-fixed-dim transition-colors" @click.prevent>{{ t('auth.forgotCode') }}</a>
          </div>

          <!-- Submit Button -->
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

    <!-- Mandatory First-Time Account Setup Modal -->
    <BaseModal
      v-model="showPasswordChangeModal"
      :title="t('auth.firstTimeSetupTitle')"
      icon="lock_reset"
      max-width="max-w-md"
      :show-close="false"
    >
      <form id="changePasswordForm" @submit.prevent="handleSaveNewPassword" class="flex flex-col gap-3">
        <p class="text-xs text-on-surface-variant leading-relaxed">
          {{ t('auth.firstTimeSetupDescription') }}
        </p>

        <div v-if="passwordChangeError" class="p-2 bg-error-container/40 border border-error text-error rounded-lg text-xs font-mono">
          {{ passwordChangeError }}
        </div>

        <BaseInput
          v-model="customUsername"
          :label="t('auth.permanentUsername')"
          placeholder="e.g. alex.morgan"
          :required="true"
          :disabled="authStore.user?.username === 'root'"
          size="sm"
        />

        <BaseInput
          v-model="newPassword"
          :label="authStore.user?.must_change_password ? t('auth.newPassword') : t('users.newPasswordOptional')"
          :placeholder="authStore.user?.must_change_password ? '••••••••' : t('users.passwordResetPlaceholder')"
          type="password"
          :required="Boolean(authStore.user?.must_change_password)"
          size="sm"
        />

        <BaseInput
          v-if="authStore.user?.must_change_password || newPassword.length > 0"
          v-model="confirmPassword"
          :label="t('auth.confirmPassword')"
          placeholder="••••••••"
          type="password"
          :required="Boolean(authStore.user?.must_change_password)"
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
          {{ isChangingPassword ? t('users.saving') : t('auth.saveAndEnter') }}
        </AppButton>
      </template>
    </BaseModal>
  </div>
</template>
