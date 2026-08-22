<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useI18n } from '@/i18n'
import { useTheme } from '@/theme'
import { AppButton, BaseInput, BaseModal } from '@/components/common'
import { usersApi } from '@/api/users'

const authStore = useAuthStore()
const router = useRouter()
const route = useRoute()
const { t, locale, setLocale } = useI18n()
const { isDark, toggleTheme } = useTheme()

const rememberedOp = localStorage.getItem('aether_remembered_operator')
const operatorId = ref(rememberedOp || 'admin')
const accessCode = ref('')
const rememberMe = ref(true)
const isSubmitting = ref(false)

// 2FA Challenge state
const is2faStep = ref(false)
const totpCode = ref('')
const isBackupCodeMode = ref(false)
const backupCode = ref('')

const errorKey = ref<string | null>(null)
const errorRawMessage = ref<string | null>(null)
const errorMessage = computed(() => {
  if (errorKey.value) {
    return t(errorKey.value)
  }
  return errorRawMessage.value
})

// Forgot Code / Access Recovery Modal
const showForgotCodeModal = ref(false)

// Mandatory First-Time Account Setup state (Multi-step Wizard)
const showPasswordChangeModal = ref(false)
const wizardStep = ref<'username' | 'password'>('username')
const customUsername = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const passwordChangeErrorKey = ref<string | null>(null)
const passwordChangeErrorParams = ref<Record<string, string | number> | undefined>(undefined)
const passwordChangeErrorRaw = ref<string | null>(null)
const passwordChangeError = computed(() => {
  if (passwordChangeErrorKey.value) {
    return t(passwordChangeErrorKey.value, passwordChangeErrorParams.value)
  }
  return passwordChangeErrorRaw.value
})
const isChangingPassword = ref(false)

const canChangeUsername = computed(() => {
  return (
    Boolean(authStore.user) &&
    authStore.user?.username !== 'root' &&
    !authStore.user?.is_username_locked
  )
})

const mustChangePassword = computed(() => {
  return Boolean(authStore.user?.must_change_password)
})

const needsOnboardingModal = computed(() => {
  return (canChangeUsername.value || mustChangePassword.value) && authStore.user?.username !== 'root'
})

function handleCancelSetup() {
  showPasswordChangeModal.value = false
  authStore.logout()
}

function goToNextStep() {
  passwordChangeErrorKey.value = null
  passwordChangeErrorRaw.value = null

  if (wizardStep.value === 'username') {
    const uname = customUsername.value.trim()
    if (uname.length < 3 || uname.length > 32) {
      passwordChangeErrorKey.value = 'auth.invalidUsernameLength'
      return
    }
    if (!/^[a-zA-Z0-9._-]+$/.test(uname)) {
      passwordChangeErrorKey.value = 'auth.invalidCredentials'
      return
    }
    if (mustChangePassword.value) {
      wizardStep.value = 'password'
    } else {
      handleSaveNewPassword()
    }
  }
}

function goToPrevStep() {
  passwordChangeErrorKey.value = null
  passwordChangeErrorRaw.value = null
  wizardStep.value = 'username'
}

function handleKeepCurrentUsername() {
  if (authStore.user?.username) {
    customUsername.value = authStore.user.username
  }
  if (mustChangePassword.value) {
    goToNextStep()
  } else {
    handleSaveNewPassword()
  }
}

// Password complexity rules check
const passwordRequirements = computed(() => {
  const cfg = authStore.authConfig
  const minLen = cfg?.min_password_length || 8
  const reqUpper = cfg?.require_uppercase ?? true
  const reqDigits = cfg?.require_digits ?? true
  const reqSpecial = cfg?.require_special ?? true

  const pwd = newPassword.value
  return {
    length: pwd.length >= minLen,
    minLength: minLen,
    upper: !reqUpper || /[A-ZА-Я]/.test(pwd),
    digits: !reqDigits || /[0-9]/.test(pwd),
    special: !reqSpecial || /[^a-zA-Z0-9а-яА-Я]/.test(pwd),
    reqUpper,
    reqDigits,
    reqSpecial
  }
})

onMounted(async () => {
  await authStore.checkAuthConfig()
  if (authStore.authConfig?.web_ui_auth === false) {
    router.push('/dashboard')
    return
  }

  if (route.query.reason === 'inactivity' || authStore.sessionExpired) {
    errorKey.value = 'auth.sessionExpiredInactivity'
    errorRawMessage.value = null
  }
})

async function handleLogin() {
  if (!operatorId.value || !accessCode.value) return
  isSubmitting.value = true
  errorKey.value = null
  errorRawMessage.value = null

  try {
    const res = await authStore.login(operatorId.value, accessCode.value, rememberMe.value)

    if (res.requires_2fa) {
      is2faStep.value = true
      totpCode.value = ''
      backupCode.value = ''
      return
    }

    if (needsOnboardingModal.value) {
      customUsername.value = authStore.user?.username || ''
      wizardStep.value = canChangeUsername.value ? 'username' : 'password'
      showPasswordChangeModal.value = true
    } else if (authStore.authConfig?.force_2fa && !authStore.user?.is_totp_enabled) {
      router.push('/settings/profile?setup_2fa=true')
    } else {
      router.push('/dashboard')
    }
  } catch (err: any) {
    console.error('Login failed:', err)
    if (err?.status === 401 || err?.i18n_key === 'core.error.unauthorized' || err?.i18n_key === 'core.auth.invalid_credentials') {
      errorKey.value = 'auth.invalidCredentials'
      errorRawMessage.value = null
    } else {
      errorKey.value = null
      errorRawMessage.value = err?.message || t('auth.invalidCredentials')
    }
  } finally {
    isSubmitting.value = false
  }
}

async function handleVerify2fa() {
  const code = isBackupCodeMode.value ? backupCode.value.trim() : totpCode.value.trim()
  if (!code) return

  isSubmitting.value = true
  errorKey.value = null
  errorRawMessage.value = null

  try {
    await authStore.verify2faLogin(
      code,
      isBackupCodeMode.value,
      rememberMe.value,
      operatorId.value
    )

    if (needsOnboardingModal.value) {
      customUsername.value = authStore.user?.username || ''
      wizardStep.value = canChangeUsername.value ? 'username' : 'password'
      showPasswordChangeModal.value = true
    } else {
      router.push('/dashboard')
    }
  } catch (err: any) {
    console.error('2FA verification failed:', err)
    if (isBackupCodeMode.value) {
      errorKey.value = 'auth.invalidBackupCode'
    } else {
      errorKey.value = 'auth.invalidTotpCode'
    }
    errorRawMessage.value = null
  } finally {
    isSubmitting.value = false
  }
}

function handleBackToCredentials() {
  is2faStep.value = false
  totpCode.value = ''
  backupCode.value = ''
  errorKey.value = null
  errorRawMessage.value = null
}

async function handleSaveNewPassword() {
  passwordChangeErrorKey.value = null
  passwordChangeErrorParams.value = undefined
  passwordChangeErrorRaw.value = null

  if (canChangeUsername.value) {
    const uname = customUsername.value.trim()
    if (uname.length < 3 || uname.length > 32) {
      passwordChangeErrorKey.value = 'auth.invalidUsernameLength'
      wizardStep.value = 'username'
      return
    }
    if (!/^[a-zA-Z0-9._-]+$/.test(uname)) {
      passwordChangeErrorKey.value = 'auth.invalidCredentials'
      wizardStep.value = 'username'
      return
    }
  }

  const isMandatoryPassword = mustChangePassword.value
  const hasPasswordInput = newPassword.value.length > 0 || confirmPassword.value.length > 0

  if (isMandatoryPassword || hasPasswordInput) {
    const reqs = passwordRequirements.value
    if (!reqs.length) {
      passwordChangeErrorKey.value = 'auth.passwordReqLength'
      passwordChangeErrorParams.value = { min: reqs.minLength }
      return
    }
    if (!reqs.upper) {
      passwordChangeErrorKey.value = 'auth.passwordReqUpper'
      return
    }
    if (!reqs.digits) {
      passwordChangeErrorKey.value = 'auth.passwordReqDigit'
      return
    }
    if (!reqs.special) {
      passwordChangeErrorKey.value = 'auth.passwordReqSpecial'
      return
    }
    if (newPassword.value !== confirmPassword.value) {
      passwordChangeErrorKey.value = 'auth.passwordsDoNotMatch'
      return
    }
  }

  isChangingPassword.value = true
  passwordChangeErrorKey.value = null
  passwordChangeErrorRaw.value = null
  try {
    if (authStore.user) {
      const payload: { username?: string; password?: string; must_change_password?: boolean; is_username_locked?: boolean } = {}
      if (canChangeUsername.value) {
        payload.username = customUsername.value.trim()
        payload.is_username_locked = true
      }
      if (hasPasswordInput) {
        payload.password = newPassword.value
        payload.must_change_password = false
      }
      const updated = await usersApi.update(authStore.user.id, payload)
      authStore.user = updated
    }
    showPasswordChangeModal.value = false
    if (authStore.authConfig?.force_2fa && !authStore.user?.is_totp_enabled) {
      router.push('/settings/profile?setup_2fa=true')
    } else {
      router.push('/dashboard')
    }
  } catch (err: any) {
    passwordChangeErrorRaw.value = err.message || t('auth.passwordChangeError')
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
        style="background-image: url('/login-bg.jpg');"
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
        <div class="w-12 h-12 rounded-xl overflow-hidden bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center shadow-glow">
          <img
            src="/logo.png"
            alt="AetherCore Logo"
            class="w-full h-full object-cover"
          />
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

        <!-- STEP 1: Credentials (Username + Password) -->
        <form v-if="!is2faStep" @submit.prevent="handleLogin" class="flex flex-col gap-4">
          <!-- Operator ID Input -->
          <BaseInput
            v-model="operatorId"
            :label="t('auth.operatorId')"
            :placeholder="t('auth.operatorIdPlaceholder')"
            icon="badge"
            :required="true"
            :autofocus="!operatorId"
          />

          <!-- Access Code Input -->
          <BaseInput
            v-model="accessCode"
            :label="t('auth.accessCode')"
            :placeholder="t('auth.accessCodePlaceholder')"
            type="password"
            icon="key"
            :required="true"
            :autofocus="Boolean(operatorId)"
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
            <a
              href="#"
              class="hover:text-primary-fixed-dim transition-colors cursor-pointer"
              @click.prevent="showForgotCodeModal = true"
            >
              {{ t('auth.forgotCode') }}
            </a>
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

        <!-- STEP 2: Two-Factor Authentication (2FA Challenge) -->
        <form v-else @submit.prevent="handleVerify2fa" class="flex flex-col gap-4">
          <div class="flex items-center gap-3 p-3 bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 rounded-lg">
            <span class="material-symbols-outlined text-primary-fixed-dim text-2xl shrink-0">verified_user</span>
            <div class="flex flex-col">
              <span class="text-xs font-bold text-on-surface">{{ isBackupCodeMode ? t('auth.backupCodeTitle') : t('auth.twoFactorTitle') }}</span>
              <span class="text-[11px] text-on-surface-variant leading-tight mt-0.5">{{ isBackupCodeMode ? t('auth.backupCodeDesc') : t('auth.twoFactorDesc') }}</span>
            </div>
          </div>

          <!-- TOTP 6-digit Code Input -->
          <div v-if="!isBackupCodeMode">
            <BaseInput
              v-model="totpCode"
              :label="t('auth.totpCode')"
              :placeholder="t('auth.totpPlaceholder')"
              icon="pin"
              :required="true"
              :autofocus="true"
              class="font-mono text-center tracking-widest text-lg font-bold"
              maxlength="8"
            />
          </div>

          <!-- Backup Recovery Code Input -->
          <div v-else>
            <BaseInput
              v-model="backupCode"
              :label="t('auth.backupCodeTitle')"
              :placeholder="t('auth.backupCodePlaceholder')"
              icon="vpn_key"
              :required="true"
              :autofocus="true"
              class="font-mono tracking-widest text-sm font-bold uppercase"
              maxlength="16"
            />
          </div>

          <!-- Switch Mode: TOTP / Backup Code -->
          <div class="flex items-center justify-between text-xs text-on-surface-variant">
            <button
              type="button"
              class="hover:text-primary-fixed-dim transition-colors cursor-pointer text-left"
              @click="isBackupCodeMode = !isBackupCodeMode; errorKey = null; errorRawMessage = null"
            >
              {{ isBackupCodeMode ? t('auth.useTotpCode') : t('auth.useBackupCode') }}
            </button>
            <button
              type="button"
              class="hover:text-on-surface transition-colors cursor-pointer text-right"
              @click="handleBackToCredentials"
            >
              {{ t('auth.backToLogin') }}
            </button>
          </div>

          <!-- Verify Button -->
          <AppButton
            type="submit"
            variant="primary"
            size="lg"
            :block="true"
            icon="check_circle"
            :loading="isSubmitting"
          >
            {{ isSubmitting ? t('auth.verifying') : t('auth.verifyAndLogin') }}
          </AppButton>
        </form>
      </div>
    </main>

    <!-- Forgot Code / Access Recovery Modal -->
    <BaseModal
      v-model="showForgotCodeModal"
      :title="t('auth.forgotCodeTitle')"
      icon="support_agent"
      max-width="max-w-md"
    >
      <div class="flex flex-col gap-4 text-xs text-on-surface leading-relaxed">
        <div class="p-4 bg-surface-container border border-outline-variant rounded-lg flex items-start gap-3">
          <span class="material-symbols-outlined text-primary-fixed-dim text-xl shrink-0 mt-0.5">info</span>
          <p class="text-on-surface leading-relaxed">
            {{ t('auth.forgotCodeDesc') }}
          </p>
        </div>
      </div>

      <template #footer>
        <AppButton
          variant="secondary"
          size="sm"
          @click="showForgotCodeModal = false"
        >
          {{ t('common.close') }}
        </AppButton>
      </template>
    </BaseModal>

    <!-- Mandatory First-Time Account Setup / Password Change Modal -->
    <BaseModal
      v-model="showPasswordChangeModal"
      :title="canChangeUsername ? t('auth.firstTimeSetupTitle') : t('auth.passwordChangeRequiredTitle')"
      icon="admin_panel_settings"
      max-width="max-w-lg"
      :scrollable="false"
      :show-close="false"
      :close-on-esc="false"
      :close-on-click-outside="false"
    >
      <div class="flex flex-col gap-4">
        <!-- Step Progress Bar & Indicators (only if both username and password changes are required) -->
        <div v-if="canChangeUsername && mustChangePassword" class="grid grid-cols-2 gap-2 border-b border-outline-variant/40 pb-3">
          <div
            class="flex items-center gap-2 p-1.5 rounded-lg transition-all"
            :class="wizardStep === 'username' ? 'bg-primary-fixed-dim/15 text-primary-fixed-dim font-bold' : 'text-on-surface-variant opacity-60'"
          >
            <span class="w-5 h-5 rounded-full flex items-center justify-center text-xs font-mono" :class="wizardStep === 'username' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'bg-surface-container-highest text-on-surface-variant'">1</span>
            <span class="text-xs">{{ t('auth.wizardStepUsername') }}</span>
          </div>
          <div
            class="flex items-center gap-2 p-1.5 rounded-lg transition-all"
            :class="wizardStep === 'password' ? 'bg-primary-fixed-dim/15 text-primary-fixed-dim font-bold' : 'text-on-surface-variant opacity-60'"
          >
            <span class="w-5 h-5 rounded-full flex items-center justify-center text-xs font-mono" :class="wizardStep === 'password' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'bg-surface-container-highest text-on-surface-variant'">2</span>
            <span class="text-xs">{{ t('auth.wizardStepPassword') }}</span>
          </div>
        </div>

        <p class="text-xs text-on-surface-variant leading-relaxed">
          {{ canChangeUsername && wizardStep === 'username' ? t('auth.wizardStepUsernameDesc') : t('auth.wizardStepPasswordDesc') }}
        </p>

        <!-- Error in Wizard -->
        <div v-if="passwordChangeError" class="p-3 bg-error-container/30 border border-error text-error text-xs rounded-lg flex items-center gap-2">
          <span class="material-symbols-outlined text-base shrink-0">error</span>
          <span>{{ passwordChangeError }}</span>
        </div>

        <!-- STEP 1: Username Setup -->
        <div v-if="canChangeUsername && wizardStep === 'username'" class="flex flex-col gap-3">
          <BaseInput
            v-model="customUsername"
            :label="t('auth.permanentUsername')"
            placeholder="e.g. operator_alex"
            icon="badge"
            :required="true"
            :autofocus="true"
          />
          <div class="flex items-center justify-between">
            <span class="text-[11px] text-on-surface-variant">{{ t('auth.usernameHintRules') }}</span>
            <button
              type="button"
              class="text-xs text-primary-fixed-dim hover:underline cursor-pointer"
              @click="handleKeepCurrentUsername"
            >
              {{ t('auth.keepCurrentUsername') }} ({{ authStore.user?.username }})
            </button>
          </div>
        </div>

        <!-- STEP 2: Password Setup -->
        <div v-if="!canChangeUsername || wizardStep === 'password'" class="flex flex-col gap-3">
          <BaseInput
            v-model="newPassword"
            :label="t('auth.newPassword')"
            type="password"
            icon="lock"
            :required="true"
            :autofocus="true"
          />
          <BaseInput
            v-model="confirmPassword"
            :label="t('auth.confirmPassword')"
            type="password"
            icon="lock_reset"
            :required="true"
          />

          <!-- Password Requirements Checklist -->
          <div class="bg-surface-container p-3 rounded-lg border border-outline-variant/60 flex flex-col gap-1.5">
            <span class="text-[11px] font-bold text-on-surface uppercase tracking-wider mb-1">{{ t('auth.passwordRequirementsTitle') }}</span>
            <div class="grid grid-cols-2 gap-1.5 text-xs">
              <div class="flex items-center gap-1.5" :class="passwordRequirements.length ? 'text-primary-fixed-dim' : 'text-on-surface-variant/60'">
                <span class="material-symbols-outlined text-sm">{{ passwordRequirements.length ? 'check_circle' : 'radio_button_unchecked' }}</span>
                <span>{{ t('auth.passwordReqLength', { min: passwordRequirements.minLength }) }}</span>
              </div>
              <div v-if="passwordRequirements.reqUpper" class="flex items-center gap-1.5" :class="passwordRequirements.upper ? 'text-primary-fixed-dim' : 'text-on-surface-variant/60'">
                <span class="material-symbols-outlined text-sm">{{ passwordRequirements.upper ? 'check_circle' : 'radio_button_unchecked' }}</span>
                <span>{{ t('auth.passwordReqUpper') }}</span>
              </div>
              <div v-if="passwordRequirements.reqDigits" class="flex items-center gap-1.5" :class="passwordRequirements.digits ? 'text-primary-fixed-dim' : 'text-on-surface-variant/60'">
                <span class="material-symbols-outlined text-sm">{{ passwordRequirements.digits ? 'check_circle' : 'radio_button_unchecked' }}</span>
                <span>{{ t('auth.passwordReqDigit') }}</span>
              </div>
              <div v-if="passwordRequirements.reqSpecial" class="flex items-center gap-1.5" :class="passwordRequirements.special ? 'text-primary-fixed-dim' : 'text-on-surface-variant/60'">
                <span class="material-symbols-outlined text-sm">{{ passwordRequirements.special ? 'check_circle' : 'radio_button_unchecked' }}</span>
                <span>{{ t('auth.passwordReqSpecial') }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <template #footer>
        <div class="flex items-center justify-between w-full">
          <AppButton
            variant="ghost"
            size="sm"
            @click="handleCancelSetup"
          >
            {{ t('common.cancel') }}
          </AppButton>

          <div class="flex items-center gap-2">
            <AppButton
              v-if="canChangeUsername && wizardStep === 'password'"
              variant="secondary"
              size="sm"
              icon="arrow_back"
              @click="goToPrevStep"
            >
              {{ t('auth.wizardBack') }}
            </AppButton>

            <AppButton
              v-if="canChangeUsername && wizardStep === 'username' && mustChangePassword"
              variant="primary"
              size="sm"
              icon="arrow_forward"
              @click="goToNextStep"
            >
              {{ t('auth.wizardNext') }}
            </AppButton>

            <AppButton
              v-else
              variant="primary"
              size="sm"
              icon="check"
              :loading="isChangingPassword"
              @click="handleSaveNewPassword"
            >
              {{ t('auth.saveAndEnter') }}
            </AppButton>
          </div>
        </div>
      </template>
    </BaseModal>
  </div>
</template>
