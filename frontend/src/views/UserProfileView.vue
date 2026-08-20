<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import { useI18n, type Locale } from '@/i18n'
import { useTheme, type ThemeMode } from '@/theme'
import { useAuthStore } from '@/stores/auth'

const { t, locale, setLocale } = useI18n()
const { theme, setTheme } = useTheme()
const authStore = useAuthStore()

// Profile Form State
const fullName = ref(authStore.user?.full_name || 'Главный администратор (Root)')
const department = ref('Network Operations')
const role = ref('Superuser')
const email = ref(authStore.user?.email || 'root@nms.local')
const timezone = ref('Europe/Minsk')

// Live Local Clock State
const currentTimeString = ref('')
let clockTimer: number | null = null

function updateClock() {
  try {
    const now = new Date()
    currentTimeString.value = now.toLocaleTimeString('en-US', {
      timeZone: timezone.value.includes('/') ? timezone.value : undefined,
      hour12: true,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    }) + ` (${timezone.value})`
  } catch {
    currentTimeString.value = new Date().toLocaleTimeString('en-US', { hour12: true }) + ` (${timezone.value})`
  }
}

onMounted(() => {
  updateClock()
  clockTimer = window.setInterval(updateClock, 1000)
})

onUnmounted(() => {
  if (clockTimer) clearInterval(clockTimer)
})

// Password Form State
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const passwordStatus = ref<string | null>(null)

// Do Not Disturb & Quiet Hours State
const activeMuteDuration = ref<'none' | '15m' | '1h' | '8h' | '24h' | 'inf'>('none')
const quietHoursEnabled = ref(false)

// Sound Preferences State
const soundInfo = ref('Soft Chime')
const soundSuccess = ref('Major Chord')
const soundWarning = ref('Double Beep')
const soundError = ref('Alarm Tone')

// Module Subscriptions State
interface ModuleSub {
  id: string
  nameKey: string
  code: string
  descKey: string
  enabled: boolean
  mute: 'none' | '15m' | '1h' | '8h' | 'inf'
  sound: string
  threshold: string
}

const moduleSubscriptions = ref<ModuleSub[]>([
  {
    id: 'core',
    nameKey: 'profile.systemCore',
    code: 'core',
    descKey: 'profile.systemCoreDesc',
    enabled: true,
    mute: 'none',
    sound: 'Default (by severity)',
    threshold: 'profile.allEvents'
  },
  {
    id: 'topology',
    nameKey: 'profile.moduleTopology',
    code: 'wasm.topology',
    descKey: 'profile.moduleTopologyDesc',
    enabled: true,
    mute: 'none',
    sound: 'Synth Chime',
    threshold: 'profile.warnAndErrors'
  }
])

// Save Notification
const savedNotice = ref(false)

function handleThemeChange(event: Event) {
  const select = event.target as HTMLSelectElement
  setTheme(select.value.toLowerCase() as ThemeMode)
}

function handleLocaleChange(event: Event) {
  const select = event.target as HTMLSelectElement
  setLocale(select.value as Locale)
}

function handleSaveProfile() {
  if (authStore.user) {
    authStore.user.full_name = fullName.value
    authStore.user.email = email.value
  }
  savedNotice.value = true
  setTimeout(() => {
    savedNotice.value = false
  }, 3000)
}

function handleChangePassword() {
  if (!currentPassword.value || !newPassword.value) {
    passwordStatus.value = 'Заполните все поля пароля'
    return
  }
  if (newPassword.value !== confirmPassword.value) {
    passwordStatus.value = 'Пароли не совпадают'
    return
  }
  passwordStatus.value = 'Пароль успешно обновлен'
  currentPassword.value = ''
  newPassword.value = ''
  confirmPassword.value = ''
  setTimeout(() => {
    passwordStatus.value = null
  }, 3000)
}

function handleAutoDetectTimezone() {
  timezone.value = Intl.DateTimeFormat().resolvedOptions().timeZone || 'Europe/Minsk'
  updateClock()
}

// Web Audio API Sound Synthesizer for live preview
function playSoundEffect(type: 'info' | 'success' | 'warning' | 'error') {
  try {
    const AudioContextClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext
    const ctx = new AudioContextClass()
    const osc = ctx.createOscillator()
    const gain = ctx.createGain()
    osc.connect(gain)
    gain.connect(ctx.destination)

    if (type === 'info') {
      osc.type = 'sine'
      osc.frequency.setValueAtTime(523.25, ctx.currentTime)
      osc.frequency.exponentialRampToValueAtTime(659.25, ctx.currentTime + 0.12)
      gain.gain.setValueAtTime(0.15, ctx.currentTime)
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.25)
      osc.start()
      osc.stop(ctx.currentTime + 0.25)
    } else if (type === 'success') {
      osc.type = 'sine'
      osc.frequency.setValueAtTime(440, ctx.currentTime)
      osc.frequency.setValueAtTime(554.37, ctx.currentTime + 0.08)
      osc.frequency.setValueAtTime(659.25, ctx.currentTime + 0.16)
      gain.gain.setValueAtTime(0.18, ctx.currentTime)
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.35)
      osc.start()
      osc.stop(ctx.currentTime + 0.35)
    } else if (type === 'warning') {
      osc.type = 'triangle'
      osc.frequency.setValueAtTime(440, ctx.currentTime)
      osc.frequency.setValueAtTime(370, ctx.currentTime + 0.12)
      gain.gain.setValueAtTime(0.2, ctx.currentTime)
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.3)
      osc.start()
      osc.stop(ctx.currentTime + 0.3)
    } else if (type === 'error') {
      osc.type = 'sawtooth'
      osc.frequency.setValueAtTime(220, ctx.currentTime)
      osc.frequency.setValueAtTime(164.81, ctx.currentTime + 0.15)
      gain.gain.setValueAtTime(0.2, ctx.currentTime)
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.35)
      osc.start()
      osc.stop(ctx.currentTime + 0.35)
    }
  } catch (e) {
    console.debug('Audio not supported or blocked:', e)
  }
}
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)] select-none">
    <!-- Top Settings Subnavigation -->
    <SettingsNav />

    <!-- Main Profile Content Canvas -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="p-lg flex flex-col gap-lg w-full">
        <!-- Top Page Header -->
        <div class="flex items-center justify-between flex-wrap gap-md">
          <div class="flex items-center gap-sm text-on-surface">
            <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
              <span class="material-symbols-outlined text-xl">account_circle</span>
            </div>
            <div>
              <h1 class="font-display-lg text-display-lg text-on-surface font-bold">
                {{ t('profile.title') }}
              </h1>
              <p class="text-xs text-on-surface-variant mt-0.5">
                {{ t('profile.subtitle') }}
              </p>
            </div>
          </div>
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-12 gap-lg">
          <!-- Left Column: User Summary & Security -->
          <div class="lg:col-span-4 flex flex-col gap-lg">
            <!-- Profile Card -->
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark flex flex-col items-center text-center gap-md">
              <div class="relative mb-2">
                <div class="w-24 h-24 rounded-full bg-surface-variant border border-primary-fixed-dim flex items-center justify-center text-2xl font-bold font-body-mono text-primary-fixed-dim shadow-glow-primary-md">
                  ГА
                </div>
                <div class="absolute bottom-1 right-1 w-4 h-4 bg-tertiary-fixed-dim rounded-full border-2 border-background animate-pulse"></div>
              </div>
              <div>
                <h2 class="font-display-lg text-display-lg text-on-surface font-bold">
                  {{ fullName }}
                </h2>
                <p class="text-xs text-primary-fixed-dim font-medium tracking-wide mt-1">
                  {{ t('profile.superuserRole') }}
                </p>
                <p class="text-[11px] text-on-surface-variant font-body-mono mt-0.5">
                  {{ t('profile.uid') }}
                </p>
              </div>

              <div class="flex items-center justify-between w-full mt-2 px-md py-2 border border-outline-variant/40 rounded-lg bg-surface-container">
                <div class="flex items-center gap-1.5">
                  <span class="w-2 h-2 rounded-full bg-tertiary-fixed-dim animate-pulse"></span>
                  <span class="text-xs text-tertiary-fixed-dim font-bold">{{ t('profile.activeStatus') }}</span>
                </div>
                <span class="text-xs text-on-surface-variant font-body-mono font-medium">{{ currentTimeString }}</span>
              </div>

              <div class="grid grid-cols-2 gap-sm w-full">
                <button
                  type="button"
                  class="h-8 bg-surface-container hover:bg-surface-variant text-on-surface border border-outline-variant rounded-lg text-xs font-semibold flex items-center justify-center gap-1.5 transition-all cursor-pointer active:scale-95"
                >
                  <span class="material-symbols-outlined text-base text-primary-fixed-dim">upload</span>
                  <span>{{ t('profile.uploadPhoto') }}</span>
                </button>
                <button
                  type="button"
                  class="h-8 bg-surface-container hover:bg-surface-variant text-on-surface border border-outline-variant rounded-lg text-xs font-semibold flex items-center justify-center gap-1.5 transition-all cursor-pointer active:scale-95"
                >
                  <span class="material-symbols-outlined text-base text-on-surface-variant">restart_alt</span>
                  <span>{{ t('profile.resetPhoto') }}</span>
                </button>
              </div>
            </div>

            <!-- Security Policies Card -->
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark flex flex-col gap-md">
              <div class="flex items-center gap-sm text-on-surface">
                <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                  <span class="material-symbols-outlined text-xl">security</span>
                </div>
                <div>
                  <h3 class="font-title-sm font-bold text-on-surface">{{ t('profile.securityPolicies') }}</h3>
                  <p class="text-xs text-on-surface-variant mt-0.5">{{ t('profile.securityPoliciesDesc') }}</p>
                </div>
              </div>

              <div v-if="passwordStatus" class="p-2 text-xs rounded font-body-mono bg-primary-fixed-dim/10 border border-primary-fixed-dim text-primary-fixed-dim">
                {{ passwordStatus }}
              </div>

              <form class="flex flex-col gap-sm" @submit.prevent="handleChangePassword">
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase">
                    {{ t('profile.currentPassword') }}
                  </label>
                  <input
                    v-model="currentPassword"
                    type="password"
                    class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg px-3 text-xs font-body-mono text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                    placeholder="••••••••••••"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase">
                    {{ t('profile.newPassword') }}
                  </label>
                  <input
                    v-model="newPassword"
                    type="password"
                    class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg px-3 text-xs font-body-mono text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                    placeholder="••••••••••••"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-label-caps text-on-surface-variant uppercase">
                    {{ t('profile.confirmNewPassword') }}
                  </label>
                  <input
                    v-model="confirmPassword"
                    type="password"
                    class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg px-3 text-xs font-body-mono text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                    placeholder="••••••••••••"
                  />
                </div>
                <div class="flex justify-end mt-xs">
                  <button
                    type="submit"
                    class="h-8 px-4 bg-primary-fixed-dim text-on-primary-fixed font-bold shadow-glow-primary-sm hover:shadow-glow-primary-md hover:bg-primary-fixed-dim/90 rounded-lg text-xs uppercase flex items-center justify-center gap-1.5 transition-all cursor-pointer active:scale-95"
                  >
                    <span class="material-symbols-outlined text-base">lock_reset</span>
                    <span>{{ t('profile.changePassword') }}</span>
                  </button>
                </div>
              </form>
            </div>

            <!-- Two-Factor Auth Card -->
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark flex flex-col gap-md">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-sm text-on-surface">
                  <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                    <span class="material-symbols-outlined text-xl">verified_user</span>
                  </div>
                  <div>
                    <h3 class="font-title-sm font-bold text-on-surface">{{ t('profile.twoFactorAuth') }}</h3>
                    <p class="text-xs text-on-surface-variant mt-0.5">TOTP</p>
                  </div>
                </div>
                <span class="px-2 py-0.5 bg-surface-variant text-[10px] font-bold text-on-surface-variant rounded uppercase font-body-mono border border-outline-variant/40">
                  Disabled
                </span>
              </div>
              <p class="text-xs text-on-surface-variant leading-relaxed">
                {{ t('profile.twoFactorDesc') }}
              </p>
              <button
                type="button"
                class="h-8 bg-surface-container hover:bg-surface-variant text-on-surface border border-outline-variant rounded-lg text-xs font-semibold uppercase flex items-center justify-center gap-1.5 transition-all cursor-pointer active:scale-95"
              >
                <span class="material-symbols-outlined text-base text-primary-fixed-dim">qr_code_2</span>
                <span>{{ t('profile.setup2fa') }}</span>
              </button>
            </div>
          </div>

          <!-- Right Column -->
          <div class="lg:col-span-8 flex flex-col gap-lg">
            <!-- Personal Information -->
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark flex flex-col gap-md">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-sm text-on-surface">
                  <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                    <span class="material-symbols-outlined text-xl">person</span>
                  </div>
                  <div>
                    <h3 class="font-title-sm font-bold text-on-surface">{{ t('profile.personalInfo') }}</h3>
                    <p class="text-xs text-on-surface-variant mt-0.5">{{ t('profile.personalInfoDesc') }}</p>
                  </div>
                </div>
                <button
                  type="button"
                  class="h-8 px-4 bg-primary-fixed-dim text-on-primary-fixed font-bold shadow-glow-primary-sm hover:shadow-glow-primary-md hover:bg-primary-fixed-dim/90 rounded-lg text-xs uppercase flex items-center gap-1.5 transition-all cursor-pointer active:scale-95"
                  @click="handleSaveProfile"
                >
                  <span class="material-symbols-outlined text-base">save</span>
                  {{ savedNotice ? t('profile.savedChanges') : t('profile.saveChanges') }}
                </button>
              </div>

              <div class="grid grid-cols-1 md:grid-cols-2 gap-md mt-sm">
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.fullName') }}</label>
                  <input
                    v-model="fullName"
                    type="text"
                    class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg px-3 text-xs text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.department') }} <span class="text-[8px] opacity-50 font-body-mono">({{ t('profile.readonly') }})</span></label>
                  <input
                    v-model="department"
                    type="text"
                    readonly
                    class="h-8 bg-surface-variant/30 border border-outline-variant/50 rounded-lg px-3 text-xs text-on-surface-variant cursor-not-allowed font-body-mono"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.role') }} <span class="text-[8px] opacity-50 font-body-mono">({{ t('profile.readonly') }})</span></label>
                  <input
                    v-model="role"
                    type="text"
                    readonly
                    class="h-8 bg-surface-variant/30 border border-outline-variant/50 rounded-lg px-3 text-xs text-on-surface-variant cursor-not-allowed font-body-mono"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.email') }}</label>
                  <input
                    v-model="email"
                    type="email"
                    class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg px-3 text-xs font-body-mono text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                  />
                </div>
              </div>
            </div>

            <!-- Appearance & Regionality -->
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark flex flex-col gap-md">
              <div class="flex items-center gap-sm text-on-surface">
                <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                  <span class="material-symbols-outlined text-xl">tune</span>
                </div>
                <div>
                  <h3 class="font-title-sm font-bold text-on-surface">{{ t('profile.appearanceRegionality') }}</h3>
                  <p class="text-xs text-on-surface-variant mt-0.5">{{ t('profile.appearanceRegionalityDesc') }}</p>
                </div>
              </div>

              <div class="grid grid-cols-1 md:grid-cols-2 gap-md mt-sm">
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.theme') }}</label>
                  <div class="relative flex items-center">
                    <select
                      :value="theme"
                      class="w-full h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-8 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                      @change="handleThemeChange"
                    >
                      <option value="dark">{{ t('profile.themeDark') }}</option>
                      <option value="light">{{ t('profile.themeLight') }}</option>
                    </select>
                    <span class="material-symbols-outlined text-base text-on-surface-variant absolute right-2.5 pointer-events-none">
                      expand_more
                    </span>
                  </div>
                </div>

                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.language') }}</label>
                  <div class="relative flex items-center">
                    <select
                      :value="locale"
                      class="w-full h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-8 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                      @change="handleLocaleChange"
                    >
                      <option value="en">English (EN)</option>
                      <option value="ru">Русский (RU)</option>
                      <option value="de" disabled>Deutsch (DE) — скоро</option>
                      <option value="es" disabled>Español (ES) — скоро</option>
                      <option value="fr" disabled>Français (FR) — скоро</option>
                      <option value="zh" disabled>中文 (ZH) — скоро</option>
                    </select>
                    <span class="material-symbols-outlined text-base text-on-surface-variant absolute right-2.5 pointer-events-none">
                      expand_more
                    </span>
                  </div>
                </div>

                <div class="flex flex-col gap-1 md:col-span-2">
                  <div class="flex items-center justify-between">
                    <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.timezone') }}</label>
                    <button
                      type="button"
                      class="flex items-center gap-1 text-primary-fixed-dim text-xs cursor-pointer hover:underline"
                      @click="handleAutoDetectTimezone"
                    >
                      <span class="material-symbols-outlined text-sm">auto_mode</span>
                      <span>{{ t('profile.autoDetect') }}</span>
                    </button>
                  </div>
                  <div class="relative flex items-center">
                    <select
                      v-model="timezone"
                      class="w-full h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-8 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer font-body-mono"
                    >
                      <option value="Europe/Minsk">Europe/Minsk (GMT+3)</option>
                      <option value="Europe/Moscow">Europe/Moscow (GMT+3)</option>
                      <option value="UTC">UTC (GMT+0)</option>
                      <option value="America/New_York">America/New_York (GMT-5)</option>
                    </select>
                    <span class="material-symbols-outlined absolute right-2.5 text-base text-on-surface-variant pointer-events-none">
                      expand_more
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <!-- Complete Notification Settings -->
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg shadow-card-dark flex flex-col gap-lg">
              <div class="flex items-start justify-between">
                <div class="flex items-center gap-sm text-on-surface">
                  <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                    <span class="material-symbols-outlined text-xl">notifications_active</span>
                  </div>
                  <div>
                    <h3 class="font-title-sm font-bold text-on-surface">{{ t('profile.notificationSettings') }}</h3>
                    <p class="text-xs text-on-surface-variant mt-0.5">{{ t('profile.notificationDesc') }}</p>
                  </div>
                </div>
              </div>

              <!-- Top Row: Do Not Disturb & Quiet Hours Grid -->
              <div class="grid grid-cols-1 md:grid-cols-2 gap-md">
                <!-- Do Not Disturb Card -->
                <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex flex-col justify-between gap-3">
                  <div class="flex items-start justify-between gap-2">
                    <div class="flex items-center gap-2.5">
                      <div class="w-8 h-8 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                        <span class="material-symbols-outlined text-base">do_not_disturb_on</span>
                      </div>
                      <div>
                        <h4 class="text-xs font-bold text-on-surface">{{ t('profile.doNotDisturb') }}</h4>
                        <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t('profile.doNotDisturbDesc') }}</p>
                      </div>
                    </div>
                    <div
                      class="flex items-center gap-1.5 px-2.5 py-0.5 rounded-full border text-[10px] font-bold uppercase shrink-0"
                      :class="activeMuteDuration !== 'none'
                        ? 'bg-warning-yellow/10 border-warning-yellow/30 text-warning-yellow'
                        : 'bg-tertiary-fixed-dim/10 border-tertiary-fixed-dim/30 text-tertiary-fixed-dim'"
                    >
                      <span
                        class="w-1.5 h-1.5 rounded-full"
                        :class="activeMuteDuration !== 'none' ? 'bg-warning-yellow' : 'bg-tertiary-fixed-dim animate-pulse'"
                      ></span>
                      <span>{{ activeMuteDuration !== 'none' ? `${t('profile.notificationsMuted')} (${activeMuteDuration})` : t('profile.notificationsActive') }}</span>
                    </div>
                  </div>

                  <div class="flex items-center justify-between flex-wrap gap-2 pt-2.5 border-t border-outline-variant/30">
                    <span class="text-xs text-on-surface-variant font-medium">{{ t('profile.muteNotifications') }}</span>
                    <div class="flex items-center gap-1">
                      <button
                        v-for="dur in (['15m', '1h', '8h', '24h'] as const)"
                        :key="dur"
                        type="button"
                        class="h-7 px-2.5 border rounded-md text-xs font-semibold transition-all cursor-pointer"
                        :class="activeMuteDuration === dur
                          ? 'bg-primary-fixed-dim text-on-primary-fixed border-primary-fixed-dim shadow-glow-primary-sm'
                          : 'bg-surface-container-highest hover:bg-surface-variant text-on-surface border-outline-variant/40'"
                        @click="activeMuteDuration = activeMuteDuration === dur ? 'none' : dur"
                      >
                        {{ dur }}
                      </button>
                      <button
                        type="button"
                        class="h-7 px-2.5 rounded-md text-xs font-semibold flex items-center gap-1 border transition-all cursor-pointer"
                        :class="activeMuteDuration === 'inf'
                          ? 'bg-primary-fixed-dim text-on-primary-fixed border-primary-fixed-dim shadow-glow-primary-sm'
                          : 'bg-surface-container-highest hover:bg-surface-variant text-on-surface border-outline-variant/40'"
                        @click="activeMuteDuration = activeMuteDuration === 'inf' ? 'none' : 'inf'"
                      >
                        <span>{{ t('profile.untilTurnedOff') }}</span>
                      </button>
                    </div>
                  </div>
                </div>

                <!-- Quiet Hours Card -->
                <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex flex-col justify-between gap-3">
                  <div class="flex items-start justify-between gap-2">
                    <div class="flex items-center gap-2.5">
                      <div class="w-8 h-8 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                        <span class="material-symbols-outlined text-base">schedule</span>
                      </div>
                      <div>
                        <h4 class="text-xs font-bold text-on-surface">{{ t('profile.quietHours') }}</h4>
                        <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t('profile.quietHoursDesc') }}</p>
                      </div>
                    </div>
                    <label class="relative inline-flex items-center cursor-pointer shrink-0 mt-0.5">
                      <input class="sr-only peer" type="checkbox" v-model="quietHoursEnabled">
                      <div class="w-10 h-5 bg-surface-container-highest rounded-full border border-outline-variant peer-checked:bg-primary-fixed-dim peer-checked:border-primary-fixed-dim transition-colors relative after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-on-surface-variant peer-checked:after:bg-on-primary peer-checked:after:translate-x-5 after:rounded-full after:h-3.5 after:w-3.5 after:transition-transform"></div>
                    </label>
                  </div>

                  <div class="flex items-center justify-between flex-wrap gap-2 pt-2.5 border-t border-outline-variant/30">
                    <span class="text-xs text-on-surface-variant font-medium">{{ t('profile.quietScheduleLabel') }}</span>
                    <span class="text-xs font-body-mono font-bold text-on-surface bg-surface-container-highest px-2.5 py-1 rounded-md border border-outline-variant/40">
                      23:00 — 07:00 (GMT+3)
                    </span>
                  </div>
                </div>
              </div>

              <!-- Module Subscriptions Section (Clean Table) -->
              <div class="bg-surface-container border border-outline-variant rounded-lg overflow-hidden flex flex-col">
                <div class="p-md border-b border-outline-variant flex items-center gap-2.5">
                  <div class="w-8 h-8 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                    <span class="material-symbols-outlined text-base">widgets</span>
                  </div>
                  <div>
                    <h4 class="text-xs font-bold text-on-surface">{{ t('profile.moduleSubscriptions') }}</h4>
                    <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t('profile.moduleSubscriptionsDesc') }}</p>
                  </div>
                </div>

                <div class="overflow-x-auto">
                  <table class="w-full text-left border-collapse">
                    <thead class="bg-surface-container-high/60 text-[10px] text-on-surface-variant uppercase font-bold border-b border-outline-variant">
                      <tr>
                        <th class="p-md">{{ t('profile.colModule') }}</th>
                        <th class="p-md">{{ t('profile.colMute') }}</th>
                        <th class="p-md">{{ t('profile.colSound') }}</th>
                        <th class="p-md">{{ t('profile.colThreshold') }}</th>
                      </tr>
                    </thead>
                    <tbody class="divide-y divide-outline-variant/30 text-xs">
                      <tr
                        v-for="mod in moduleSubscriptions"
                        :key="mod.id"
                        class="hover:bg-surface-variant/20 transition-colors"
                      >
                        <!-- Column 1: Module & Description -->
                        <td class="p-md">
                          <div class="flex items-center gap-3">
                            <input
                              v-model="mod.enabled"
                              class="rounded border-outline-variant bg-surface-container-highest text-primary-fixed-dim focus:ring-0 cursor-pointer"
                              type="checkbox"
                            >
                            <div>
                              <div class="flex items-center gap-2">
                                <span class="font-bold text-on-surface">{{ t(mod.nameKey) }}</span>
                                <span class="bg-surface-variant text-[10px] px-1.5 py-0.5 rounded font-body-mono text-on-surface-variant">{{ mod.code }}</span>
                              </div>
                              <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t(mod.descKey) }}</p>
                            </div>
                          </div>
                        </td>

                        <!-- Column 2: Temporary Mute Buttons -->
                        <td class="p-md">
                          <div class="flex items-center gap-1">
                            <button
                              v-for="m in (['15m', '1h', '8h', 'inf'] as const)"
                              :key="m"
                              type="button"
                              class="px-2 py-0.5 rounded text-[10px] font-bold border border-outline-variant transition-colors cursor-pointer"
                              :class="mod.mute === m
                                ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm'
                                : 'bg-surface-container-highest hover:bg-surface-variant text-on-surface'"
                              @click="mod.mute = mod.mute === m ? 'none' : m"
                            >
                              {{ m === 'inf' ? '∞' : m }}
                            </button>
                          </div>
                        </td>

                        <!-- Column 3: Sound Select & Preview -->
                        <td class="p-md">
                          <div class="flex items-center gap-2">
                            <div class="relative flex items-center">
                              <select
                                v-model="mod.sound"
                                class="h-7 bg-surface-container-highest border border-outline-variant rounded-lg pl-2 pr-6 text-[10px] text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                              >
                                <option>Default (by severity)</option>
                                <option>Synth Chime</option>
                                <option>Futuristic Blip</option>
                                <option>Mute sound</option>
                              </select>
                              <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1 pointer-events-none">expand_more</span>
                            </div>
                            <button
                              type="button"
                              class="w-7 h-7 rounded-lg bg-surface-container-highest hover:bg-surface-variant border border-outline-variant flex items-center justify-center text-primary-fixed-dim transition-colors cursor-pointer"
                              title="Прослушать сигнал"
                              @click="playSoundEffect('info')"
                            >
                              <span class="material-symbols-outlined text-base">play_arrow</span>
                            </button>
                          </div>
                        </td>

                        <!-- Column 4: Severity Threshold -->
                        <td class="p-md">
                          <div class="relative flex items-center">
                            <select
                              v-model="mod.threshold"
                              class="h-7 bg-surface-container-highest border border-outline-variant rounded-lg pl-2 pr-6 text-[10px] text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                            >
                              <option value="profile.allEvents">{{ t('profile.allEvents') }}</option>
                              <option value="profile.warnAndErrors">{{ t('profile.warnAndErrors') }}</option>
                              <option value="profile.criticalOnly">{{ t('profile.criticalOnly') }}</option>
                              <option value="profile.silentMode">{{ t('profile.silentMode') }}</option>
                            </select>
                            <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1 pointer-events-none">expand_more</span>
                          </div>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </div>

              <!-- Sound Signals Section -->
              <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex flex-col gap-sm">
                <div class="flex items-center gap-2.5">
                  <div class="w-8 h-8 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                    <span class="material-symbols-outlined text-base">volume_up</span>
                  </div>
                  <div>
                    <h4 class="text-xs font-bold text-on-surface">{{ t('profile.soundSignals') }}</h4>
                    <p class="text-[11px] text-on-surface-variant mt-0.5">{{ t('profile.soundSignalsDesc') }}</p>
                  </div>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-3 mt-1">
                  <div class="flex items-center justify-between p-2.5 bg-surface-container-low border border-outline-variant/50 rounded-lg">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-primary-fixed-dim text-base">info</span>
                      <span class="text-xs font-semibold text-on-surface">{{ t('profile.infoSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="relative flex items-center">
                        <select
                          v-model="soundInfo"
                          class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-2.5 pr-7 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                        >
                          <option>Soft Chime</option>
                          <option>Digital Click</option>
                          <option>Hologram Whir</option>
                          <option>Gentle Bell</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 pointer-events-none">expand_more</span>
                      </div>
                      <button
                        type="button"
                        class="w-8 h-8 rounded-lg bg-surface-container-highest hover:bg-surface-variant border border-outline-variant flex items-center justify-center text-primary-fixed-dim transition-colors cursor-pointer"
                        title="Прослушать сигнал"
                        @click="playSoundEffect('info')"
                      >
                        <span class="material-symbols-outlined text-base">play_arrow</span>
                      </button>
                    </div>
                  </div>

                  <div class="flex items-center justify-between p-2.5 bg-surface-container-low border border-outline-variant/50 rounded-lg">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-tertiary-fixed-dim text-base">check_circle</span>
                      <span class="text-xs font-semibold text-on-surface">{{ t('profile.successSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="relative flex items-center">
                        <select
                          v-model="soundSuccess"
                          class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-2.5 pr-7 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                        >
                          <option>Major Chord</option>
                          <option>Ascending Harp</option>
                          <option>Success Ping</option>
                          <option>Power Up</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 pointer-events-none">expand_more</span>
                      </div>
                      <button
                        type="button"
                        class="w-8 h-8 rounded-lg bg-surface-container-highest hover:bg-surface-variant border border-outline-variant flex items-center justify-center text-tertiary-fixed-dim transition-colors cursor-pointer"
                        title="Прослушать сигнал"
                        @click="playSoundEffect('success')"
                      >
                        <span class="material-symbols-outlined text-base">play_arrow</span>
                      </button>
                    </div>
                  </div>

                  <div class="flex items-center justify-between p-2.5 bg-surface-container-low border border-outline-variant/50 rounded-lg">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-warning-yellow text-base">warning</span>
                      <span class="text-xs font-semibold text-on-surface">{{ t('profile.warningSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="relative flex items-center">
                        <select
                          v-model="soundWarning"
                          class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-2.5 pr-7 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                        >
                          <option>Double Beep</option>
                          <option>Low Drone</option>
                          <option>Caution Radar</option>
                          <option>Warning Clack</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 pointer-events-none">expand_more</span>
                      </div>
                      <button
                        type="button"
                        class="w-8 h-8 rounded-lg bg-surface-container-highest hover:bg-surface-variant border border-outline-variant flex items-center justify-center text-warning-yellow transition-colors cursor-pointer"
                        title="Прослушать сигнал"
                        @click="playSoundEffect('warning')"
                      >
                        <span class="material-symbols-outlined text-base">play_arrow</span>
                      </button>
                    </div>
                  </div>

                  <div class="flex items-center justify-between p-2.5 bg-surface-container-low border border-outline-variant/50 rounded-lg">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-error text-base">error</span>
                      <span class="text-xs font-semibold text-on-surface">{{ t('profile.errorSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="relative flex items-center">
                        <select
                          v-model="soundError"
                          class="h-8 bg-surface-container-highest border border-outline-variant rounded-lg pl-2.5 pr-7 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                        >
                          <option>Alarm Tone</option>
                          <option>Heavy Klaxon</option>
                          <option>Critical Siren</option>
                          <option>System Failure</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 pointer-events-none">expand_more</span>
                      </div>
                      <button
                        type="button"
                        class="w-8 h-8 rounded-lg bg-surface-container-highest hover:bg-surface-variant border border-outline-variant flex items-center justify-center text-error transition-colors cursor-pointer"
                        title="Прослушать сигнал"
                        @click="playSoundEffect('error')"
                      >
                        <span class="material-symbols-outlined text-base">play_arrow</span>
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Bottom: Active Sessions -->
            <div class="bg-surface-container-low border border-outline-variant rounded-lg overflow-hidden shadow-card-dark">
              <div class="p-md border-b border-outline-variant bg-surface-container flex items-center justify-between flex-wrap gap-2">
                <div class="flex items-center gap-sm text-on-surface">
                  <div class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0">
                    <span class="material-symbols-outlined text-xl">devices</span>
                  </div>
                  <div>
                    <h3 class="font-title-sm font-bold text-on-surface">{{ t('profile.activeSessions') }}</h3>
                    <p class="text-xs text-on-surface-variant mt-0.5">{{ t('profile.activeSessionsDesc') }}</p>
                  </div>
                </div>
                <div class="flex gap-2">
                  <button
                    type="button"
                    class="h-8 bg-error-container/20 border border-error/30 text-error hover:bg-error-container/40 px-3 rounded-lg text-xs font-bold uppercase transition-all flex items-center gap-1.5 cursor-pointer active:scale-95"
                  >
                    <span class="material-symbols-outlined text-base">security</span>
                    {{ t('profile.terminateOthers') }}
                  </button>
                </div>
              </div>

              <div class="overflow-x-auto">
                <table class="w-full text-left border-collapse">
                  <thead class="bg-surface-container text-[10px] text-on-surface-variant uppercase font-bold border-b border-outline-variant">
                    <tr>
                      <th class="p-md">{{ t('profile.ipAddress') }}</th>
                      <th class="p-md">{{ t('profile.deviceBrowser') }}</th>
                      <th class="p-md">{{ t('profile.lastSeen') }}</th>
                      <th class="p-md text-right">{{ t('common.actions') }}</th>
                    </tr>
                  </thead>
                  <tbody class="text-xs font-body-mono divide-y divide-outline-variant/30">
                    <tr class="hover:bg-surface-variant/20 transition-colors">
                      <td class="p-md">
                        <div class="flex items-center gap-2">
                          <span class="text-on-surface font-bold">127.0.0.1</span>
                          <span class="bg-tertiary-fixed-dim/20 text-tertiary-fixed-dim border border-tertiary-fixed-dim/30 text-[9px] px-1.5 py-0.2 rounded font-bold font-body-mono uppercase">
                            {{ t('profile.currentSession') }}
                          </span>
                        </div>
                      </td>
                      <td class="p-md text-on-surface">Edge (Windows)</td>
                      <td class="p-md text-on-surface-variant">8/19/2026, 12:25:30 AM</td>
                      <td class="p-md text-right">
                        <button
                          type="button"
                          class="h-7 text-[10px] font-bold uppercase text-error border border-error/30 px-2.5 rounded-lg hover:bg-error-container/20 transition-colors cursor-pointer active:scale-95"
                        >
                          {{ t('profile.revoke') }}
                        </button>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>
      </div>
    </main>
  </div>
</template>
