<script setup lang="ts">
import { ref } from 'vue'
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

// Password Form State
const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const passwordStatus = ref<string | null>(null)

// Do Not Disturb State
const activeMuteDuration = ref('15m')
const quietHoursEnabled = ref(false)

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
                <div class="w-24 h-24 rounded-full bg-surface-variant border border-primary-fixed-dim flex items-center justify-center text-2xl font-bold text-primary-fixed-dim shadow-glow-primary-md">
                  ГА
                </div>
                <div class="absolute bottom-1 right-1 w-4 h-4 bg-tertiary-fixed-dim rounded-full border-2 border-background animate-pulse"></div>
              </div>
              <div>
                <h2 class="font-display-lg text-display-lg text-on-surface font-bold">
                  {{ fullName }}
                </h2>
                <p class="text-xs text-on-surface-variant font-body-mono uppercase tracking-widest mt-1">
                  {{ t('profile.superuserRole') }}
                </p>
                <p class="text-[10px] text-on-surface-variant font-body-mono mt-1">
                  {{ t('profile.uid') }}
                </p>
              </div>

              <div class="flex items-center justify-between w-full mt-2 px-md py-2 border border-outline-variant/40 rounded-lg bg-surface-container">
                <div class="flex items-center gap-2">
                  <span class="text-[10px] text-on-surface-variant uppercase font-bold">Status:</span>
                  <span class="text-[10px] text-tertiary-fixed-dim uppercase font-bold">{{ t('profile.activeStatus') }}</span>
                </div>
                <span class="text-[10px] text-on-surface-variant font-body-mono">12:39:58 AM (Europe/Minsk)</span>
              </div>

              <div class="grid grid-cols-2 gap-sm w-full">
                <button
                  type="button"
                  class="bg-surface-container hover:bg-surface-variant text-on-surface border border-outline-variant py-2.5 rounded-lg text-xs font-bold uppercase flex items-center justify-center gap-2 transition-all cursor-pointer active:scale-95"
                >
                  <span class="material-symbols-outlined text-sm">upload</span>
                  {{ t('profile.uploadPhoto') }}
                </button>
                <button
                  type="button"
                  class="bg-surface-container hover:bg-surface-variant text-on-surface border border-outline-variant py-2.5 rounded-lg text-xs font-bold uppercase flex items-center justify-center gap-2 transition-all cursor-pointer active:scale-95"
                >
                  <span class="material-symbols-outlined text-sm">restart_alt</span>
                  {{ t('profile.resetPhoto') }}
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
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">
                    {{ t('profile.currentPassword') }}
                  </label>
                  <input
                    v-model="currentPassword"
                    type="password"
                    class="bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-2 text-sm font-body-mono text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                    placeholder="••••••••••••"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">
                    {{ t('profile.newPassword') }}
                  </label>
                  <input
                    v-model="newPassword"
                    type="password"
                    class="bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-2 text-sm font-body-mono text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                    placeholder="••••••••••••"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">
                    {{ t('profile.confirmNewPassword') }}
                  </label>
                  <input
                    v-model="confirmPassword"
                    type="password"
                    class="bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-2 text-sm font-body-mono text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                    placeholder="••••••••••••"
                  />
                </div>
                <button
                  type="submit"
                  class="bg-primary-fixed-dim text-on-primary-fixed font-bold shadow-glow-primary-sm hover:shadow-glow-primary-md hover:bg-primary-fixed-dim/90 py-2.5 rounded-lg text-xs uppercase flex items-center justify-center gap-2 mt-2 transition-all cursor-pointer active:scale-95"
                >
                  <span class="material-symbols-outlined text-sm">lock_reset</span>
                  {{ t('profile.changePassword') }}
                </button>
              </form>
            </div>

            <!-- 2FA Card -->
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
                <span class="px-2 py-0.5 bg-surface-variant text-[10px] font-bold text-outline-variant rounded uppercase">
                  Disabled
                </span>
              </div>
              <p class="text-xs text-on-surface-variant">
                {{ t('profile.twoFactorDesc') }}
              </p>
              <button
                type="button"
                class="bg-surface-container hover:bg-surface-variant text-on-surface border border-outline-variant py-2.5 rounded-lg text-xs font-bold uppercase flex items-center justify-center gap-2 transition-all cursor-pointer active:scale-95"
              >
                <span class="material-symbols-outlined text-sm">qr_code_2</span>
                {{ t('profile.setup2fa') }}
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
                  class="bg-primary-fixed-dim text-on-primary-fixed font-bold shadow-glow-primary-sm hover:shadow-glow-primary-md hover:bg-primary-fixed-dim/90 px-4 py-2 rounded-lg text-xs uppercase flex items-center gap-2 transition-all cursor-pointer active:scale-95"
                  @click="handleSaveProfile"
                >
                  <span class="material-symbols-outlined text-sm">save</span>
                  {{ savedNotice ? t('profile.savedChanges') : t('profile.saveChanges') }}
                </button>
              </div>

              <div class="grid grid-cols-1 md:grid-cols-2 gap-md mt-sm">
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.fullName') }}</label>
                  <input
                    v-model="fullName"
                    type="text"
                    class="bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-2 text-sm text-on-surface focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.department') }} <span class="text-[8px] opacity-50">({{ t('profile.readonly') }})</span></label>
                  <input
                    v-model="department"
                    type="text"
                    readonly
                    class="bg-surface-variant/30 border border-outline-variant/50 rounded-lg px-3 py-2 text-sm text-on-surface-variant cursor-not-allowed font-body-mono"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.role') }} <span class="text-[8px] opacity-50">({{ t('profile.readonly') }})</span></label>
                  <input
                    v-model="role"
                    type="text"
                    readonly
                    class="bg-surface-variant/30 border border-outline-variant/50 rounded-lg px-3 py-2 text-sm text-on-surface-variant cursor-not-allowed font-body-mono"
                  />
                </div>
                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.email') }}</label>
                  <input
                    v-model="email"
                    type="email"
                    class="bg-surface-container-highest border border-outline-variant rounded-lg px-3 py-2 text-sm text-on-surface font-body-mono focus:ring-1 focus:ring-primary-fixed-dim outline-none placeholder:text-on-surface-variant/50"
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
                  <div class="relative">
                    <select
                      :value="theme"
                      class="w-full bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-8 py-2 text-sm text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                      @change="handleThemeChange"
                    >
                      <option value="dark">{{ t('profile.themeDark') }}</option>
                      <option value="light">{{ t('profile.themeLight') }}</option>
                    </select>
                    <span class="material-symbols-outlined text-sm text-on-surface-variant absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none">
                      expand_more
                    </span>
                  </div>
                </div>

                <div class="flex flex-col gap-1">
                  <label class="text-[10px] font-bold text-on-surface-variant uppercase">{{ t('profile.language') }}</label>
                  <div class="relative">
                    <select
                      :value="locale"
                      class="w-full bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-8 py-2 text-sm text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer"
                      @change="handleLocaleChange"
                    >
                      <option value="en">English (EN)</option>
                      <option value="ru">Russian (RU)</option>
                    </select>
                    <span class="material-symbols-outlined text-sm text-on-surface-variant absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none">
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
                  <div class="relative">
                    <select
                      v-model="timezone"
                      class="w-full bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-10 py-2 text-sm text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer font-body-mono text-xs"
                    >
                      <option value="Europe/Minsk">Europe/Minsk (GMT+3)</option>
                      <option value="Europe/Moscow">Europe/Moscow (GMT+3)</option>
                      <option value="UTC">UTC (GMT+0)</option>
                      <option value="America/New_York">America/New_York (GMT-5)</option>
                    </select>
                    <span class="material-symbols-outlined absolute right-2.5 top-1/2 -translate-y-1/2 text-sm text-primary-fixed-dim pointer-events-none">
                      auto_mode
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <!-- Notification Settings -->
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

              <!-- Do Not Disturb Section -->
              <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex flex-col gap-md">
                <div class="flex items-start justify-between flex-wrap gap-2">
                  <div class="flex items-start gap-3">
                    <span class="material-symbols-outlined text-on-surface-variant mt-0.5">do_not_disturb_on</span>
                    <div>
                      <h4 class="text-sm font-bold text-on-surface">{{ t('profile.doNotDisturb') }}</h4>
                      <p class="text-[10px] text-on-surface-variant">{{ t('profile.doNotDisturbDesc') }}</p>
                    </div>
                  </div>
                  <div class="flex items-center gap-2 bg-tertiary-fixed-dim/10 px-2 py-1 rounded-full border border-tertiary-fixed-dim/30">
                    <div class="w-1.5 h-1.5 rounded-full bg-tertiary-fixed-dim"></div>
                    <span class="text-[10px] text-tertiary-fixed-dim font-bold uppercase">{{ t('profile.notificationsActive') }}</span>
                  </div>
                </div>

                <div class="flex flex-wrap items-center gap-2">
                  <span class="text-[10px] text-on-surface-variant uppercase font-bold mr-1">{{ t('profile.muteNotifications') }}</span>
                  <button
                    v-for="dur in ['15m', '1h', '8h', '24h']"
                    :key="dur"
                    type="button"
                    class="py-1.5 px-3 border border-outline-variant rounded-lg text-xs font-bold transition-all cursor-pointer"
                    :class="activeMuteDuration === dur
                      ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm'
                      : 'bg-surface-container hover:bg-surface-variant text-on-surface'"
                    @click="activeMuteDuration = dur"
                  >
                    {{ dur === '15m' ? '15 min' : dur === '1h' ? '1 hour' : dur === '8h' ? '8 hours' : '24 hours' }}
                  </button>
                  <button
                    type="button"
                    class="h-8 rounded-lg text-xs font-bold flex items-center gap-1.5 px-3 transition-all cursor-pointer"
                    :class="activeMuteDuration === 'inf'
                      ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm'
                      : 'bg-surface-container hover:bg-surface-variant text-on-surface border border-outline-variant'"
                    @click="activeMuteDuration = 'inf'"
                  >
                    <span class="material-symbols-outlined text-sm">pause_circle</span>
                    {{ t('profile.untilTurnedOff') }}
                  </button>
                </div>
              </div>

              <!-- Quiet Hours Section -->
              <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <span class="material-symbols-outlined text-on-surface-variant">schedule</span>
                  <div>
                    <h4 class="text-sm font-bold text-on-surface">{{ t('profile.quietHours') }}</h4>
                    <p class="text-[10px] text-on-surface-variant">{{ t('profile.quietHoursDesc') }}</p>
                  </div>
                </div>
                <div
                  class="w-10 h-5 rounded-full relative cursor-pointer transition-colors border border-outline-variant"
                  :class="quietHoursEnabled ? 'bg-primary-fixed-dim' : 'bg-surface-container-highest'"
                  @click="quietHoursEnabled = !quietHoursEnabled"
                >
                  <div
                    class="absolute top-0.5 w-3.5 h-3.5 rounded-full transition-all"
                    :class="quietHoursEnabled ? 'right-0.5 bg-on-primary-fixed' : 'left-0.5 bg-on-surface-variant'"
                  ></div>
                </div>
              </div>

              <!-- Module Subscriptions Section -->
              <div class="flex flex-col gap-md">
                <div>
                  <h4 class="text-sm font-bold text-on-surface">{{ t('profile.moduleSubscriptions') }}</h4>
                  <p class="text-[10px] text-on-surface-variant">{{ t('profile.moduleSubscriptionsDesc') }}</p>
                </div>
                <div class="p-md bg-surface-container border border-outline-variant rounded-lg flex items-center justify-between flex-wrap gap-md">
                  <div class="flex items-center gap-3 shrink-0">
                    <input checked class="rounded border-outline-variant bg-surface-container-highest text-primary-fixed-dim focus:ring-primary-fixed-dim cursor-pointer" type="checkbox">
                    <div>
                      <div class="flex items-center gap-2">
                        <span class="text-sm font-bold text-on-surface">{{ t('profile.systemCore') }}</span>
                        <span class="bg-surface-variant text-[10px] px-1.5 py-0.5 rounded font-body-mono text-on-surface-variant">core</span>
                      </div>
                      <p class="text-[10px] text-on-surface-variant">{{ t('profile.systemCoreDesc') }}</p>
                    </div>
                  </div>
                  <div class="flex flex-wrap items-center gap-lg">
                    <div class="flex flex-col gap-1">
                      <span class="text-[10px] text-on-surface-variant uppercase font-bold">{{ t('profile.moduleMute') }}</span>
                      <div class="flex items-center gap-1">
                        <button type="button" class="px-2.5 py-1 bg-primary-fixed-dim text-on-primary-fixed text-[10px] font-bold shadow-glow-primary-sm rounded-lg cursor-pointer">15m</button>
                        <button type="button" class="px-2.5 py-1 bg-surface-container-highest border border-outline-variant text-[10px] font-bold text-on-surface hover:bg-surface-variant rounded-lg transition-colors cursor-pointer">1h</button>
                        <button type="button" class="px-2.5 py-1 bg-surface-container-highest border border-outline-variant text-[10px] font-bold text-on-surface hover:bg-surface-variant rounded-lg transition-colors cursor-pointer">8h</button>
                        <button type="button" class="px-2.5 py-1 bg-surface-container-highest border border-outline-variant text-[10px] font-bold text-on-surface hover:bg-surface-variant rounded-lg transition-colors cursor-pointer">∞</button>
                      </div>
                    </div>
                    <div class="flex flex-col gap-1">
                      <span class="text-[10px] text-on-surface-variant uppercase font-bold">{{ t('profile.moduleSound') }}</span>
                      <div class="flex items-center gap-2">
                        <div class="relative">
                          <select class="bg-surface-container-highest border border-outline-variant rounded-lg pl-2 pr-6 py-1 text-[10px] text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer">
                            <option selected>Default (by severity)</option>
                            <option>Synth Chime</option>
                            <option>Futuristic Blip</option>
                            <option>Subtle Pulse</option>
                            <option>Mute sound</option>
                          </select>
                          <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 top-1/2 -translate-y-1/2 pointer-events-none">expand_more</span>
                        </div>
                        <span class="material-symbols-outlined text-primary-fixed-dim text-sm cursor-pointer hover:scale-110 transition-transform">play_arrow</span>
                      </div>
                    </div>
                    <div class="flex flex-col gap-1">
                      <span class="text-[10px] text-on-surface-variant uppercase font-bold">{{ t('profile.severityThreshold') }}</span>
                      <div class="relative">
                        <select class="bg-surface-container-highest border border-outline-variant rounded-lg pl-2 pr-6 py-1 text-[10px] text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer">
                          <option selected>{{ t('profile.allEvents') }}</option>
                          <option>{{ t('profile.warnAndErrors') }}</option>
                          <option>{{ t('profile.criticalOnly') }}</option>
                          <option>{{ t('profile.silentMode') }}</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 top-1/2 -translate-y-1/2 pointer-events-none">expand_more</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Sound Signals Section -->
              <div class="flex flex-col gap-md">
                <div class="flex items-center gap-2">
                  <span class="material-symbols-outlined text-on-surface-variant">volume_up</span>
                  <div>
                    <h4 class="text-sm font-bold text-on-surface">{{ t('profile.soundSignals') }}</h4>
                    <p class="text-[10px] text-on-surface-variant">{{ t('profile.soundSignalsDesc') }}</p>
                  </div>
                </div>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                  <div class="flex items-center justify-between p-3 bg-surface-container border border-outline-variant rounded-lg">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-primary-fixed-dim text-sm">info</span>
                      <span class="text-xs font-semibold text-on-surface">{{ t('profile.infoSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="relative">
                        <select class="bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-7 py-1 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer">
                          <option selected>Soft Chime</option>
                          <option>Digital Click</option>
                          <option>Hologram Whir</option>
                          <option>Gentle Bell</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 top-1/2 -translate-y-1/2 pointer-events-none">expand_more</span>
                      </div>
                      <span class="material-symbols-outlined text-sm cursor-pointer hover:text-primary-fixed-dim hover:scale-110 transition-all text-primary-fixed-dim">play_arrow</span>
                    </div>
                  </div>

                  <div class="flex items-center justify-between p-3 bg-surface-container border border-outline-variant rounded-lg">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-tertiary-fixed-dim text-sm">check_circle</span>
                      <span class="text-xs font-semibold text-on-surface">{{ t('profile.successSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="relative">
                        <select class="bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-7 py-1 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer">
                          <option selected>Major Chord</option>
                          <option>Ascending Harp</option>
                          <option>Success Ping</option>
                          <option>Power Up</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 top-1/2 -translate-y-1/2 pointer-events-none">expand_more</span>
                      </div>
                      <span class="material-symbols-outlined text-sm cursor-pointer hover:text-tertiary-fixed-dim hover:scale-110 transition-all text-tertiary-fixed-dim">play_arrow</span>
                    </div>
                  </div>

                  <div class="flex items-center justify-between p-3 bg-surface-container border border-outline-variant rounded-lg">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-warning-yellow text-sm">warning</span>
                      <span class="text-xs font-semibold text-on-surface">{{ t('profile.warningSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="relative">
                        <select class="bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-7 py-1 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer">
                          <option selected>Double Beep</option>
                          <option>Low Drone</option>
                          <option>Caution Radar</option>
                          <option>Warning Clack</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 top-1/2 -translate-y-1/2 pointer-events-none">expand_more</span>
                      </div>
                      <span class="material-symbols-outlined text-sm cursor-pointer hover:text-warning-yellow hover:scale-110 transition-all text-warning-yellow">play_arrow</span>
                    </div>
                  </div>

                  <div class="flex items-center justify-between p-3 bg-surface-container border border-outline-variant rounded-lg">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-error text-sm">error</span>
                      <span class="text-xs font-semibold text-on-surface">{{ t('profile.errorSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <div class="relative">
                        <select class="bg-surface-container-highest border border-outline-variant rounded-lg pl-3 pr-7 py-1 text-xs text-on-surface appearance-none focus:ring-1 focus:ring-primary-fixed-dim outline-none cursor-pointer">
                          <option selected>Alarm Tone</option>
                          <option>Heavy Klaxon</option>
                          <option>Critical Siren</option>
                          <option>System Failure</option>
                        </select>
                        <span class="material-symbols-outlined text-xs text-on-surface-variant absolute right-1.5 top-1/2 -translate-y-1/2 pointer-events-none">expand_more</span>
                      </div>
                      <span class="material-symbols-outlined text-sm cursor-pointer hover:text-error hover:scale-110 transition-all text-error">play_arrow</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Bottom: Active Sessions -->
            <div class="bg-surface-container-low border border-outline-variant rounded-lg overflow-hidden shadow-card-dark">
              <div class="p-md border-b border-outline-variant bg-surface-container flex items-center justify-between flex-wrap gap-2">
                <div class="flex items-center gap-sm text-on-surface">
                  <div class="w-10 h-10 rounded-lg bg-tertiary-fixed-dim/10 border border-tertiary-fixed-dim/30 flex items-center justify-center text-tertiary-fixed-dim shrink-0">
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
                    class="bg-error-container/20 border border-error/30 text-error hover:bg-error-container/40 px-4 py-2 rounded-lg text-xs font-bold uppercase transition-all flex items-center gap-1 cursor-pointer active:scale-95"
                  >
                    <span class="material-symbols-outlined text-[16px]">security</span>
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
                          <span class="bg-tertiary-fixed-dim/20 text-tertiary-fixed-dim px-1.5 py-0.5 rounded text-[8px] font-bold">
                            {{ t('profile.currentSession') }}
                          </span>
                        </div>
                      </td>
                      <td class="p-md text-on-surface">Edge (Windows)</td>
                      <td class="p-md text-on-surface-variant">8/19/2026, 12:25:30 AM</td>
                      <td class="p-md text-right">
                        <button
                          type="button"
                          class="text-[10px] font-bold uppercase text-error border border-error/30 px-2.5 py-1 rounded-lg hover:bg-error-container/20 transition-colors cursor-pointer active:scale-95"
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
