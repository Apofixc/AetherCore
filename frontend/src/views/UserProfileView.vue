<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import {
  PageHeader,
  BaseCard,
  AppButton,
  BaseInput,
  BaseSelect,
  BaseSwitch,
  StatusBadge
} from '@/components/common'
import { useI18n, type Locale } from '@/i18n'
import { useTheme, type ThemeMode } from '@/theme'
import { useAuthStore } from '@/stores/auth'
import { usersApi } from '@/api/users'
import { settingsApi } from '@/api/settings'
import { getUserInitials } from '@/utils/user'

const { t, locale, setLocale } = useI18n()
const { theme, setTheme } = useTheme()
const authStore = useAuthStore()

// Profile Form State
const fullName = ref(authStore.user?.full_name || authStore.user?.username || 'Admin')
const department = ref('Core Operations')
const role = ref('Superuser')
const email = ref(authStore.user?.email || 'root@aethercore.local')
const timezone = ref(typeof Intl !== 'undefined' && Intl.DateTimeFormat().resolvedOptions().timeZone ? Intl.DateTimeFormat().resolvedOptions().timeZone : 'UTC')
const timeFormat = ref<'24h_sec' | '24h_min' | '12h_sec' | '12h_min' | 'iso'>('24h_sec')

// Avatar & Photo Upload State
const avatar = ref<string | null>(null)
const avatarStatus = ref<string | null>(null)
const isAvatarError = ref(false)
const fileInputRef = ref<HTMLInputElement | null>(null)

const userInitials = computed(() => getUserInitials(fullName.value, authStore.user?.username))
const userUid = computed(() => authStore.user?.id ? `UID: ${authStore.user.id.slice(0, 8).toUpperCase()}` : t('profile.uid'))
const userRoleText = computed(() => authStore.user?.is_superuser ? t('profile.superuserRole') : (role.value || 'Operator'))

// Live Local Clock State
const currentTimeString = ref('')
let clockTimer: number | null = null

function updateClock() {
  try {
    const now = new Date()
    const tz = timezone.value || 'UTC'
    const fmt = timeFormat.value || '24h_sec'

    if (fmt === 'iso') {
      const formatter = new Intl.DateTimeFormat('en-CA', {
        timeZone: tz,
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false
      })
      const parts = formatter.formatToParts(now)
      const getPart = (type: string) => parts.find((p) => p.type === type)?.value || ''
      currentTimeString.value = `${getPart('year')}-${getPart('month')}-${getPart('day')} ${getPart('hour')}:${getPart('minute')}:${getPart('second')} (${tz})`
    } else if (fmt === '24h_min') {
      currentTimeString.value =
        now.toLocaleTimeString(locale.value === 'ru' ? 'ru-RU' : 'en-US', {
          timeZone: tz,
          hour12: false,
          hour: '2-digit',
          minute: '2-digit'
        }) + ` (${tz})`
    } else if (fmt === '12h_sec') {
      currentTimeString.value =
        now.toLocaleTimeString('en-US', {
          timeZone: tz,
          hour12: true,
          hour: '2-digit',
          minute: '2-digit',
          second: '2-digit'
        }) + ` (${tz})`
    } else if (fmt === '12h_min') {
      currentTimeString.value =
        now.toLocaleTimeString('en-US', {
          timeZone: tz,
          hour12: true,
          hour: '2-digit',
          minute: '2-digit'
        }) + ` (${tz})`
    } else {
      // 24h_sec default
      currentTimeString.value =
        now.toLocaleTimeString(locale.value === 'ru' ? 'ru-RU' : 'en-US', {
          timeZone: tz,
          hour12: false,
          hour: '2-digit',
          minute: '2-digit',
          second: '2-digit'
        }) + ` (${tz})`
    }
  } catch {
    currentTimeString.value =
      new Date().toLocaleTimeString('en-US', { hour12: false }) + ` (${timezone.value})`
  }
}

watch([timezone, timeFormat], () => {
  updateClock()
})

// Password Form State
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const passwordStatus = ref<string | null>(null)
const isPasswordError = ref(false)

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

const moduleSubscriptions = ref<ModuleSub[]>([])

// Save Notification
const savedNotice = ref(false)

async function loadPreferences() {
  try {
    const serverPrefs = await settingsApi.getUserPreferences()
    if (serverPrefs) {
      if (serverPrefs.avatar) avatar.value = serverPrefs.avatar
      if (serverPrefs.timezone) timezone.value = serverPrefs.timezone
      if (serverPrefs.time_format) timeFormat.value = serverPrefs.time_format as any
      if (serverPrefs.department) department.value = serverPrefs.department
      if (serverPrefs.active_mute_duration) activeMuteDuration.value = serverPrefs.active_mute_duration as any
      if (typeof serverPrefs.quiet_hours_enabled === 'boolean') quietHoursEnabled.value = serverPrefs.quiet_hours_enabled
      if (serverPrefs.sound_info) soundInfo.value = serverPrefs.sound_info
      if (serverPrefs.sound_success) soundSuccess.value = serverPrefs.sound_success
      if (serverPrefs.sound_warning) soundWarning.value = serverPrefs.sound_warning
      if (serverPrefs.sound_error) soundError.value = serverPrefs.sound_error
      if (Array.isArray(serverPrefs.module_subscriptions)) {
        moduleSubscriptions.value = serverPrefs.module_subscriptions.map((m) => ({
          id: m.id,
          nameKey: m.name_key,
          code: m.code,
          descKey: m.desc_key,
          enabled: m.enabled,
          mute: m.mute as any,
          sound: m.sound,
          threshold: m.threshold
        }))
      }
      if (serverPrefs.theme && ['dark', 'light', 'system'].includes(serverPrefs.theme)) {
        setTheme(serverPrefs.theme as ThemeMode)
      }
      if (serverPrefs.locale && ['ru', 'en'].includes(serverPrefs.locale)) {
        setLocale(serverPrefs.locale as Locale)
      }
    }
  } catch (err) {
    console.debug('Could not load user preferences from server:', err)
  }
}

watch(
  () => authStore.user,
  (u) => {
    if (u) {
      if (u.full_name) fullName.value = u.full_name
      if (u.email) email.value = u.email
      if (u.is_superuser) {
        role.value = 'Superuser'
      } else if (u.roles && u.roles.length > 0) {
        role.value = u.roles[0].charAt(0).toUpperCase() + u.roles[0].slice(1)
      }
    }
  },
  { immediate: true }
)

onMounted(async () => {
  await loadPreferences()
  if (authStore.isAuthenticated && !authStore.user) {
    await authStore.fetchUser()
  }
  updateClock()
  clockTimer = window.setInterval(updateClock, 1000)
})

onUnmounted(() => {
  if (clockTimer) clearInterval(clockTimer)
})

function triggerPhotoUpload() {
  fileInputRef.value?.click()
}

function handlePhotoUpload(event: Event) {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return

  if (!file.type.startsWith('image/')) {
    avatarStatus.value = t('profile.photoFormatError')
    isAvatarError.value = true
    return
  }

  if (file.size > 2 * 1024 * 1024) {
    avatarStatus.value = t('profile.photoSizeError')
    isAvatarError.value = true
    return
  }

  const reader = new FileReader()
  reader.onload = (e) => {
    const img = new Image()
    img.onload = async () => {
      const canvas = document.createElement('canvas')
      const size = 256
      canvas.width = size
      canvas.height = size
      const ctx = canvas.getContext('2d')
      if (ctx) {
        const minDim = Math.min(img.width, img.height)
        const sx = (img.width - minDim) / 2
        const sy = (img.height - minDim) / 2
        ctx.drawImage(img, sx, sy, minDim, minDim, 0, 0, size, size)
        const dataUrl = canvas.toDataURL('image/jpeg', 0.85)
        avatar.value = dataUrl
        isAvatarError.value = false
        try {
          await settingsApi.updateUserPreferences({ avatar: dataUrl })
          avatarStatus.value = t('profile.photoUpdated')
          setTimeout(() => {
            avatarStatus.value = null
          }, 3000)
        } catch (err) {
          console.error('Failed to save avatar:', err)
        }
      }
    }
    img.src = e.target?.result as string
  }
  reader.readAsDataURL(file)
  target.value = ''
}

async function handleResetPhoto() {
  avatar.value = null
  isAvatarError.value = false
  try {
    await settingsApi.updateUserPreferences({ avatar: '' })
    avatarStatus.value = t('profile.photoRemoved')
    setTimeout(() => {
      avatarStatus.value = null
    }, 3000)
  } catch (err) {
    console.error('Failed to reset avatar:', err)
  }
}

async function handleSaveProfile() {
  if (authStore.user?.id) {
    try {
      await usersApi.update(authStore.user.id, {
        full_name: fullName.value,
        email: email.value
      })
      await authStore.fetchUser()
    } catch (err) {
      console.warn('Could not update profile via API, applying locally:', err)
      if (authStore.user) {
        authStore.user.full_name = fullName.value
        authStore.user.email = email.value
      }
    }
  }

  const prefsPayload = {
    avatar: avatar.value || '',
    timezone: timezone.value,
    time_format: timeFormat.value,
    theme: theme.value,
    locale: locale.value,
    department: department.value,
    active_mute_duration: activeMuteDuration.value,
    quiet_hours_enabled: quietHoursEnabled.value,
    quiet_schedule: '23:00 — 07:00 (GMT+3)',
    sound_info: soundInfo.value,
    sound_success: soundSuccess.value,
    sound_warning: soundWarning.value,
    sound_error: soundError.value,
    module_subscriptions: moduleSubscriptions.value.map((m) => ({
      id: m.id,
      name_key: m.nameKey,
      code: m.code,
      desc_key: m.descKey,
      enabled: m.enabled,
      mute: m.mute,
      sound: m.sound,
      threshold: m.threshold
    })),
    sidebar_collapsed: false
  }

  // Сохраняем на сервере в SQLite (kv_store)
  try {
    await settingsApi.updateUserPreferences(prefsPayload)
  } catch (err) {
    console.error('Could not save user preferences to server:', err)
  }

  savedNotice.value = true
  setTimeout(() => {
    savedNotice.value = false
  }, 3000)
}

async function handleChangePassword() {
  if (!currentPassword.value || !newPassword.value || !confirmPassword.value) {
    passwordStatus.value = t('auth.fillAllPasswordFields')
    isPasswordError.value = true
    return
  }
  if (newPassword.value !== confirmPassword.value) {
    passwordStatus.value = t('auth.passwordsDoNotMatch')
    isPasswordError.value = true
    return
  }
  if (authStore.user?.id) {
    try {
      await usersApi.update(authStore.user.id, {
        password: newPassword.value,
        current_password: currentPassword.value
      })
      passwordStatus.value = t('auth.passwordChangeSuccess')
      isPasswordError.value = false
      currentPassword.value = ''
      newPassword.value = ''
      confirmPassword.value = ''
      setTimeout(() => {
        passwordStatus.value = null
      }, 3000)
    } catch (err: any) {
      isPasswordError.value = true
      const errMsg = err.response?.data?.message || err.message || ''
      if (errMsg.includes('current_password') || errMsg.toLowerCase().includes('current password')) {
        passwordStatus.value = t('auth.invalidCurrentPassword')
      } else if (errMsg.includes('complexity') || errMsg.includes('length')) {
        passwordStatus.value = t('auth.passwordComplexityError')
      } else {
        passwordStatus.value = errMsg || t('auth.passwordChangeError')
      }
    }
  }
}

async function handleAutoDetectTimezone() {
  const detected = (typeof Intl !== 'undefined' && Intl.DateTimeFormat().resolvedOptions().timeZone) || 'UTC'
  timezone.value = detected
  updateClock()
  try {
    await settingsApi.updateUserPreferences({ timezone: detected })
  } catch (err) {
    console.debug('Could not auto-save detected timezone to server:', err)
  }
}

function getOffsetMinutes(tz: string): number {
  try {
    const now = new Date()
    const str = now.toLocaleString('en-US', { timeZone: tz })
    const targetDate = new Date(str)
    const utcStr = now.toLocaleString('en-US', { timeZone: 'UTC' })
    const utcDate = new Date(utcStr)
    return Math.round((targetDate.getTime() - utcDate.getTime()) / 60000)
  } catch {
    return 0
  }
}

function formatGmtOffset(offsetMinutes: number): string {
  const sign = offsetMinutes >= 0 ? '+' : '-'
  const abs = Math.abs(offsetMinutes)
  const hours = Math.floor(abs / 60)
  const mins = abs % 60
  if (mins === 0) {
    return `GMT${sign}${hours}`
  }
  return `GMT${sign}${hours}:${mins.toString().padStart(2, '0')}`
}

const FALLBACK_TIMEZONES = [
  'UTC',
  'Europe/London',
  'Europe/Berlin',
  'Europe/Paris',
  'Europe/Kyiv',
  'Europe/Minsk',
  'Europe/Moscow',
  'Europe/Samara',
  'Europe/Yekaterinburg',
  'Europe/Omsk',
  'Europe/Novosibirsk',
  'Europe/Krasnoyarsk',
  'Europe/Irkutsk',
  'Europe/Yakutsk',
  'Europe/Vladivostok',
  'Europe/Magadan',
  'Europe/Kamchatka',
  'Asia/Dubai',
  'Asia/Tashkent',
  'Asia/Almaty',
  'Asia/Bangkok',
  'Asia/Singapore',
  'Asia/Hong_Kong',
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Asia/Seoul',
  'Australia/Sydney',
  'Pacific/Auckland',
  'Pacific/Honolulu',
  'America/Anchorage',
  'America/Los_Angeles',
  'America/Denver',
  'America/Chicago',
  'America/New_York',
  'America/Toronto',
  'America/Sao_Paulo',
  'America/Buenos_Aires',
  'Atlantic/Reykjavik'
]

const timezoneOptions = computed(() => {
  let list: string[] = []
  if (typeof Intl !== 'undefined' && typeof (Intl as any).supportedValuesOf === 'function') {
    try {
      list = (Intl as any).supportedValuesOf('timeZone')
    } catch {
      list = FALLBACK_TIMEZONES
    }
  } else {
    list = FALLBACK_TIMEZONES
  }

  const uniqueTzs = new Set(['UTC', ...list])
  if (timezone.value) {
    uniqueTzs.add(timezone.value)
  }

  const optionsWithOffset = Array.from(uniqueTzs).map((tz) => {
    const offsetMin = getOffsetMinutes(tz)
    const offsetStr = formatGmtOffset(offsetMin)
    return {
      value: tz,
      label: `${tz} (${offsetStr})`,
      offsetMin
    }
  })

  optionsWithOffset.sort((a, b) => {
    if (a.offsetMin !== b.offsetMin) {
      return a.offsetMin - b.offsetMin
    }
    return a.value.localeCompare(b.value)
  })

  return optionsWithOffset.map(({ value, label }) => ({ value, label }))
})

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

const themeOptions = computed(() => [
  { value: 'dark', label: t('profile.themeDark') },
  { value: 'light', label: t('profile.themeLight') },
  { value: 'system', label: t('profile.themeSystem') }
])

const languageOptions = computed(() => [
  { value: 'en', label: 'English (EN)' },
  { value: 'ru', label: 'Русский (RU)' },
  { value: 'de', label: `Deutsch (DE) — ${t('profile.comingSoon')}`, disabled: true },
  { value: 'es', label: `Español (ES) — ${t('profile.comingSoon')}`, disabled: true },
  { value: 'fr', label: `Français (FR) — ${t('profile.comingSoon')}`, disabled: true },
  { value: 'zh', label: `中文 (ZH) — ${t('profile.comingSoon')}`, disabled: true }
])

const timeFormatOptions = computed(() => [
  { value: '24h_sec', label: t('profile.timeFormat24Sec') },
  { value: '24h_min', label: t('profile.timeFormat24Min') },
  { value: '12h_sec', label: t('profile.timeFormat12Sec') },
  { value: '12h_min', label: t('profile.timeFormat12Min') },
  { value: 'iso', label: t('profile.timeFormatIso') }
])


async function handleTimeFormatChange(val: string) {
  timeFormat.value = val as any
  updateClock()
  try {
    await settingsApi.updateUserPreferences({ time_format: val })
  } catch (err) {
    console.debug('Could not auto-save time format:', err)
  }
}

async function handleTimezoneChange(val: string) {
  timezone.value = val
  updateClock()
  try {
    await settingsApi.updateUserPreferences({ timezone: val })
  } catch (err) {
    console.debug('Could not auto-save timezone:', err)
  }
}

const thresholdOptions = computed(() => [
  { value: 'profile.allEvents', label: t('profile.allEvents') },
  { value: 'profile.warnAndErrors', label: t('profile.warnAndErrors') },
  { value: 'profile.criticalOnly', label: t('profile.criticalOnly') },
  { value: 'profile.silentMode', label: t('profile.silentMode') }
])

const soundOptions = [
  'Default (by severity)',
  'Synth Chime',
  'Futuristic Blip',
  'Mute sound'
]

const infoSoundOptions = ['Soft Chime', 'Digital Click', 'Hologram Whir', 'Gentle Bell']
const successSoundOptions = ['Major Chord', 'Ascending Harp', 'Success Ping', 'Power Up']
const warningSoundOptions = ['Double Beep', 'Low Drone', 'Caution Radar', 'Warning Clack']
const errorSoundOptions = ['Alarm Tone', 'Heavy Klaxon', 'Critical Siren', 'System Failure']
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)] select-none">
    <!-- Top Settings Subnavigation -->
    <SettingsNav />

    <!-- Main Profile Content Canvas -->
    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="p-lg flex flex-col gap-lg w-full">
        <!-- Top Page Header -->
        <PageHeader
          :title="t('profile.title')"
          :subtitle="t('profile.subtitle')"
          icon="account_circle"
        />

        <div class="grid grid-cols-1 lg:grid-cols-12 gap-lg">
          <!-- Left Column: User Summary & Security -->
          <div class="lg:col-span-4 flex flex-col gap-lg">
            <!-- Profile Card -->
            <div class="bg-surface-container-low border border-outline-variant p-lg rounded-lg flex flex-col items-center text-center gap-md shadow-card-dark">
              <div class="relative w-24 h-24 mx-auto">
                <div class="w-24 h-24 rounded-full bg-surface-variant border border-primary-fixed-dim flex items-center justify-center text-2xl font-bold font-mono text-primary-fixed-dim shadow-glow-primary-md overflow-hidden">
                  <img v-if="avatar" :src="avatar" alt="Avatar" class="w-full h-full object-cover" />
                  <span v-else>{{ userInitials }}</span>
                </div>
                <div class="absolute bottom-1 right-1 w-4 h-4 bg-tertiary-fixed-dim rounded-full border-2 border-background animate-pulse"></div>
              </div>
              <div class="flex flex-col items-center">
                <h2 class="font-display-lg text-display-lg text-on-surface font-bold">
                  {{ fullName }}
                </h2>
                <p class="text-xs text-primary-fixed-dim font-medium tracking-wide mt-1">
                  {{ userRoleText }}
                </p>
                <p class="text-[11px] text-on-surface-variant font-mono mt-0.5">
                  {{ userUid }}
                </p>
              </div>

              <div class="flex items-center justify-between w-full px-md py-2 border border-outline-variant/60 rounded-lg bg-surface-container">
                <div class="flex items-center gap-1.5">
                  <span class="w-2 h-2 rounded-full bg-tertiary-fixed-dim animate-pulse"></span>
                  <span class="text-xs font-bold text-tertiary-fixed-dim uppercase tracking-wider">{{ t('profile.activeStatus') }}</span>
                </div>
                <span class="text-xs text-on-surface-variant font-mono font-medium">{{ currentTimeString }}</span>
              </div>

              <div v-if="avatarStatus" class="w-full p-2 text-xs rounded-lg font-mono border text-center" :class="isAvatarError ? 'bg-error-container/20 border-error/40 text-error' : 'bg-primary-fixed-dim/10 border-primary-fixed-dim/30 text-primary-fixed-dim'">
                {{ avatarStatus }}
              </div>

              <input
                ref="fileInputRef"
                type="file"
                accept="image/png, image/jpeg, image/webp"
                class="hidden"
                @change="handlePhotoUpload"
              />

              <div class="grid grid-cols-2 gap-sm w-full">
                <AppButton
                  variant="outline"
                  size="sm"
                  icon="upload"
                  @click="triggerPhotoUpload"
                >
                  {{ t('profile.uploadPhoto') }}
                </AppButton>
                <AppButton
                  variant="outline"
                  size="sm"
                  icon="restart_alt"
                  :disabled="!avatar"
                  @click="handleResetPhoto"
                >
                  {{ t('profile.resetPhoto') }}
                </AppButton>
              </div>
            </div>

            <!-- Security Policies Card -->
            <BaseCard
              :title="t('profile.securityPolicies')"
              :subtitle="t('profile.securityPoliciesDesc')"
              icon="security"
            >
              <div v-if="passwordStatus" class="p-2.5 mb-3 text-xs rounded-xl font-mono border" :class="isPasswordError ? 'bg-error-container/20 border-error/40 text-error' : 'bg-primary-fixed-dim/10 border-primary-fixed-dim/30 text-primary-fixed-dim'">
                {{ passwordStatus }}
              </div>

              <form class="flex flex-col gap-3" @submit.prevent="handleChangePassword">
                <BaseInput
                  v-model="currentPassword"
                  type="password"
                  :label="t('profile.currentPassword')"
                  placeholder="••••••••••••"
                  size="sm"
                />
                <BaseInput
                  v-model="newPassword"
                  type="password"
                  :label="t('profile.newPassword')"
                  placeholder="••••••••••••"
                  size="sm"
                />
                <BaseInput
                  v-model="confirmPassword"
                  type="password"
                  :label="t('profile.confirmNewPassword')"
                  placeholder="••••••••••••"
                  size="sm"
                />
                <div class="flex justify-end mt-1">
                  <AppButton
                    variant="primary"
                    size="sm"
                    type="submit"
                    icon="lock_reset"
                  >
                    {{ t('profile.changePassword') }}
                  </AppButton>
                </div>
              </form>
            </BaseCard>

            <!-- Two-Factor Auth Card -->
            <BaseCard
              :title="t('profile.twoFactorAuth')"
              subtitle="TOTP"
              icon="verified_user"
              badge="Disabled"
              badge-variant="neutral"
            >
              <p class="text-xs text-on-surface-variant leading-relaxed mb-3">
                {{ t('profile.twoFactorDesc') }}
              </p>
              <AppButton
                variant="outline"
                size="sm"
                icon="qr_code_2"
                :block="true"
              >
                {{ t('profile.setup2fa') }}
              </AppButton>
            </BaseCard>
          </div>

          <!-- Right Column -->
          <div class="lg:col-span-8 flex flex-col gap-lg">
            <!-- Personal Information -->
            <BaseCard
              :title="t('profile.personalInfo')"
              :subtitle="t('profile.personalInfoDesc')"
              icon="person"
            >
              <template #headerActions>
                <AppButton
                  variant="primary"
                  size="sm"
                  icon="save"
                  @click="handleSaveProfile"
                >
                  {{ savedNotice ? t('profile.savedChanges') : t('profile.saveChanges') }}
                </AppButton>
              </template>

              <div class="grid grid-cols-1 md:grid-cols-2 gap-md">
                <BaseInput
                  v-model="fullName"
                  :label="t('profile.fullName')"
                  size="sm"
                />
                <BaseInput
                  v-model="department"
                  :label="`${t('profile.department')} (${t('profile.readonly')})`"
                  :readonly="true"
                  :disabled="true"
                  size="sm"
                />
                <BaseInput
                  v-model="role"
                  :label="`${t('profile.role')} (${t('profile.readonly')})`"
                  :readonly="true"
                  :disabled="true"
                  size="sm"
                />
                <BaseInput
                  v-model="email"
                  type="email"
                  :label="t('profile.email')"
                  size="sm"
                />
              </div>
            </BaseCard>

            <!-- Appearance & Regionality -->
            <BaseCard
              :title="t('profile.appearanceRegionality')"
              :subtitle="t('profile.appearanceRegionalityDesc')"
              icon="tune"
              :overflow-visible="true"
              class="z-20 relative"
            >
              <div class="grid grid-cols-1 md:grid-cols-2 gap-md">
                <BaseSelect
                  :model-value="theme"
                  :label="t('profile.theme')"
                  :options="themeOptions"
                  size="sm"
                  @update:model-value="(val) => setTheme(val as ThemeMode)"
                />

                <BaseSelect
                  :model-value="locale"
                  :label="t('profile.language')"
                  :options="languageOptions"
                  size="sm"
                  @update:model-value="(val) => setLocale(val as Locale)"
                />

                <BaseSelect
                  :model-value="timeFormat"
                  :label="t('profile.timeFormat')"
                  :options="timeFormatOptions"
                  size="sm"
                  @update:model-value="handleTimeFormatChange"
                />

                <BaseSelect
                  :model-value="timezone"
                  :label="t('profile.timezone')"
                  :options="timezoneOptions"
                  :searchable="true"
                  :search-placeholder="t('profile.timezoneSearchPlaceholder')"
                  size="sm"
                  @update:model-value="handleTimezoneChange"
                >
                  <template #labelRight>
                    <button
                      type="button"
                      class="flex items-center gap-1 text-primary-fixed-dim text-xs cursor-pointer hover:underline"
                      @click="handleAutoDetectTimezone"
                    >
                      <span class="material-symbols-outlined text-sm">auto_mode</span>
                      <span>{{ t('profile.autoDetect') }}</span>
                    </button>
                  </template>
                </BaseSelect>
              </div>
            </BaseCard>

            <!-- Complete Notification Settings -->
            <BaseCard
              :title="t('profile.notificationSettings')"
              :subtitle="t('profile.notificationDesc')"
              icon="notifications_active"
            >
              <div class="flex flex-col gap-lg">
                <!-- Top Row: Do Not Disturb & Quiet Hours Grid -->
                <div class="grid grid-cols-1 md:grid-cols-2 gap-md">
                  <!-- Do Not Disturb Card -->
                  <div class="p-md bg-surface-container-highest/30 border border-outline-variant/50 rounded-xl flex flex-col justify-between gap-3">
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
                      <StatusBadge
                        :variant="activeMuteDuration !== 'none' ? 'warning' : 'success'"
                        :pulse="activeMuteDuration === 'none'"
                        :dot="true"
                        size="xs"
                      >
                        {{ activeMuteDuration !== 'none' ? `${t('profile.notificationsMuted')} (${activeMuteDuration})` : t('profile.notificationsActive') }}
                      </StatusBadge>
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
                  <div class="p-md bg-surface-container-highest/30 border border-outline-variant/50 rounded-xl flex flex-col justify-between gap-3">
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
                      <BaseSwitch
                        v-model="quietHoursEnabled"
                        size="sm"
                        class="!p-0"
                      />
                    </div>

                    <div class="flex items-center justify-between flex-wrap gap-2 pt-2.5 border-t border-outline-variant/30">
                      <span class="text-xs text-on-surface-variant font-medium">{{ t('profile.quietScheduleLabel') }}</span>
                      <span class="text-xs font-mono font-bold text-on-surface bg-surface-container-highest px-2.5 py-1 rounded-md border border-outline-variant/40">
                        23:00 — 07:00 (GMT+3)
                      </span>
                    </div>
                  </div>
                </div>

                <!-- Module Subscriptions Section -->
                <div class="bg-surface-container border border-outline-variant/60 rounded-xl overflow-hidden flex flex-col">
                  <div class="p-md border-b border-outline-variant/60 flex items-center gap-2.5">
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
                      <thead class="bg-surface-container-high/60 text-[10px] text-on-surface-variant uppercase font-bold border-b border-outline-variant/60">
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
                                  <span class="bg-surface-variant text-[10px] px-1.5 py-0.5 rounded font-mono text-on-surface-variant">{{ mod.code }}</span>
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
                              <div class="w-40">
                                <BaseSelect
                                  v-model="mod.sound"
                                  :options="soundOptions"
                                  size="sm"
                                />
                              </div>
                              <AppButton
                                variant="outline"
                                size="xs"
                                icon="play_arrow"
                                :title="t('profile.listenSignal')"
                                @click="playSoundEffect('info')"
                              />
                            </div>
                          </td>

                          <!-- Column 4: Severity Threshold -->
                          <td class="p-md">
                            <div class="w-40">
                              <BaseSelect
                                v-model="mod.threshold"
                                :options="thresholdOptions"
                                size="sm"
                              />
                            </div>
                          </td>
                        </tr>
                      </tbody>
                    </table>
                  </div>
                </div>

                <!-- Sound Signals Section -->
                <div class="p-md bg-surface-container border border-outline-variant/60 rounded-xl flex flex-col gap-sm">
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
                    <div class="flex items-center justify-between p-2.5 bg-surface-container-low border border-outline-variant/50 rounded-xl">
                      <div class="flex items-center gap-2">
                        <span class="material-symbols-outlined text-primary-fixed-dim text-base">info</span>
                        <span class="text-xs font-semibold text-on-surface">{{ t('profile.infoSeverity') }}</span>
                      </div>
                      <div class="flex items-center gap-2">
                        <div class="w-36">
                          <BaseSelect
                            v-model="soundInfo"
                            :options="infoSoundOptions"
                            size="sm"
                          />
                        </div>
                        <AppButton
                          variant="outline"
                          size="xs"
                          icon="play_arrow"
                          :title="t('profile.listenSignal')"
                          @click="playSoundEffect('info')"
                        />
                      </div>
                    </div>

                    <div class="flex items-center justify-between p-2.5 bg-surface-container-low border border-outline-variant/50 rounded-xl">
                      <div class="flex items-center gap-2">
                        <span class="material-symbols-outlined text-tertiary-fixed-dim text-base">check_circle</span>
                        <span class="text-xs font-semibold text-on-surface">{{ t('profile.successSeverity') }}</span>
                      </div>
                      <div class="flex items-center gap-2">
                        <div class="w-36">
                          <BaseSelect
                            v-model="soundSuccess"
                            :options="successSoundOptions"
                            size="sm"
                          />
                        </div>
                        <AppButton
                          variant="outline"
                          size="xs"
                          icon="play_arrow"
                          :title="t('profile.listenSignal')"
                          @click="playSoundEffect('success')"
                        />
                      </div>
                    </div>

                    <div class="flex items-center justify-between p-2.5 bg-surface-container-low border border-outline-variant/50 rounded-xl">
                      <div class="flex items-center gap-2">
                        <span class="material-symbols-outlined text-warning-yellow text-base">warning</span>
                        <span class="text-xs font-semibold text-on-surface">{{ t('profile.warningSeverity') }}</span>
                      </div>
                      <div class="flex items-center gap-2">
                        <div class="w-36">
                          <BaseSelect
                            v-model="soundWarning"
                            :options="warningSoundOptions"
                            size="sm"
                          />
                        </div>
                        <AppButton
                          variant="outline"
                          size="xs"
                          icon="play_arrow"
                          :title="t('profile.listenSignal')"
                          @click="playSoundEffect('warning')"
                        />
                      </div>
                    </div>

                    <div class="flex items-center justify-between p-2.5 bg-surface-container-low border border-outline-variant/50 rounded-xl">
                      <div class="flex items-center gap-2">
                        <span class="material-symbols-outlined text-error text-base">error</span>
                        <span class="text-xs font-semibold text-on-surface">{{ t('profile.errorSeverity') }}</span>
                      </div>
                      <div class="flex items-center gap-2">
                        <div class="w-36">
                          <BaseSelect
                            v-model="soundError"
                            :options="errorSoundOptions"
                            size="sm"
                          />
                        </div>
                        <AppButton
                          variant="outline"
                          size="xs"
                          icon="play_arrow"
                          :title="t('profile.listenSignal')"
                          @click="playSoundEffect('error')"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </BaseCard>

            <!-- Bottom: Active Sessions -->
            <BaseCard
              :title="t('profile.activeSessions')"
              :subtitle="t('profile.activeSessionsDesc')"
              icon="devices_other"
              :no-padding="true"
            >
              <template #headerActions>
                <AppButton
                  variant="danger"
                  size="xs"
                  icon="shield"
                >
                  {{ t('profile.terminateOthers') }}
                </AppButton>
                <AppButton
                  variant="outline"
                  size="xs"
                  icon="logout"
                  @click="authStore.logout"
                >
                  {{ t('profile.allLogout') }}
                </AppButton>
              </template>

              <div class="overflow-x-auto">
                <table class="w-full text-left border-collapse">
                  <thead class="bg-surface-container-high/70 text-[10px] text-on-surface-variant uppercase font-bold border-b border-outline-variant/60">
                    <tr>
                      <th class="p-md">{{ t('profile.ipAddress') }}</th>
                      <th class="p-md">{{ t('profile.deviceBrowser') }}</th>
                      <th class="p-md">{{ t('profile.lastSeen') }}</th>
                      <th class="p-md text-right">{{ t('common.actions') }}</th>
                    </tr>
                  </thead>
                  <tbody class="text-xs font-mono divide-y divide-outline-variant/30">
                    <tr class="hover:bg-surface-variant/20 transition-colors">
                      <td class="p-md">
                        <div class="flex items-center gap-2">
                          <span class="text-on-surface font-bold">127.0.0.1</span>
                          <StatusBadge variant="success" :pulse="true" :dot="true" size="xs">
                            {{ t('profile.currentSession') }}
                          </StatusBadge>
                        </div>
                      </td>
                      <td class="p-md text-on-surface">Edge (Windows)</td>
                      <td class="p-md text-on-surface-variant">8/20/2026, 9:30:32 PM</td>
                      <td class="p-md text-right">
                        <AppButton
                          variant="danger"
                          size="xs"
                        >
                          {{ t('profile.revoke') }}
                        </AppButton>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </BaseCard>
          </div>
        </div>
      </div>
    </main>
  </div>
</template>
