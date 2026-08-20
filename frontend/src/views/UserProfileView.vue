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
      <div class="p-lg grid grid-cols-12 gap-lg max-w-[1600px] mx-auto">
        <!-- Left Column -->
        <div class="col-span-12 lg:col-span-3 flex flex-col gap-lg">
          <!-- Profile Header Card -->
          <div class="bg-surface-container border border-outline-variant p-xl flex flex-col items-center text-center rounded-xl">
            <div class="relative mb-md">
              <div class="w-32 h-32 rounded-full border-2 border-primary-fixed-dim p-1">
                <div class="w-full h-full rounded-full overflow-hidden bg-surface-variant flex items-center justify-center">
                  <span class="material-symbols-outlined text-[64px] text-on-surface-variant">account_circle</span>
                </div>
              </div>
              <div class="absolute bottom-1 right-1 w-6 h-6 bg-tertiary-fixed-dim rounded-full border-4 border-surface-container"></div>
            </div>

            <h2 class="text-display-lg font-display-lg text-on-surface font-bold">
              {{ fullName }}
            </h2>
            <p class="text-sm text-on-surface-variant font-body-mono mt-1">Superuser</p>
            <p class="text-xs text-outline-variant font-body-mono mt-1">UID: 0000-0000-ROOT</p>

            <div class="flex items-center justify-between w-full mt-xl pt-md border-t border-outline-variant/30">
              <div class="flex items-center gap-2">
                <span class="text-xs text-on-surface-variant">Status:</span>
                <span class="text-xs font-bold text-tertiary-fixed-dim">Active</span>
              </div>
              <span class="text-[10px] text-outline-variant font-body-mono">09:35:49 PM (Europe/Minsk)</span>
            </div>

            <div class="grid grid-cols-2 gap-md w-full mt-lg">
              <button
                type="button"
                class="flex items-center justify-center gap-2 py-2 rounded-xl border border-outline-variant text-sm font-semibold hover:bg-surface-variant transition-colors cursor-pointer"
              >
                <span class="material-symbols-outlined text-sm">upload</span>
                <span>Upload</span>
              </button>
              <button
                type="button"
                class="flex items-center justify-center gap-2 py-2 rounded-xl border border-outline-variant text-sm font-semibold hover:bg-surface-variant transition-colors cursor-pointer"
              >
                <span class="material-symbols-outlined text-sm">restart_alt</span>
                <span>Reset</span>
              </button>
            </div>
          </div>

          <!-- Security Policies Card -->
          <div class="bg-surface-container border border-outline-variant p-xl rounded-xl">
            <div class="flex items-center gap-2 mb-lg">
              <span class="material-symbols-outlined text-on-surface">security</span>
              <h3 class="font-title-sm text-on-surface font-bold">Security Policies</h3>
            </div>

            <div v-if="passwordStatus" class="mb-3 p-2 text-xs rounded font-body-mono bg-primary-fixed-dim/10 border border-primary-fixed-dim text-primary-fixed-dim">
              {{ passwordStatus }}
            </div>

            <form class="flex flex-col gap-md" @submit.prevent="handleChangePassword">
              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  Current Password
                </label>
                <input
                  v-model="currentPassword"
                  type="password"
                  class="w-full bg-surface border border-outline-variant rounded px-md py-2 text-sm text-on-surface focus:ring-1 focus:ring-primary-fixed-dim rounded-xl"
                />
              </div>
              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  New Password
                </label>
                <input
                  v-model="newPassword"
                  type="password"
                  class="w-full bg-surface border border-outline-variant rounded px-md py-2 text-sm text-on-surface focus:ring-1 focus:ring-primary-fixed-dim rounded-xl"
                />
              </div>
              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  Confirm New Password
                </label>
                <input
                  v-model="confirmPassword"
                  type="password"
                  class="w-full bg-surface border border-outline-variant rounded px-md py-2 text-sm text-on-surface focus:ring-1 focus:ring-primary-fixed-dim rounded-xl"
                />
              </div>
              <button
                type="submit"
                class="w-full mt-md py-2 bg-surface-variant hover:bg-surface-variant/80 text-on-surface rounded text-sm font-semibold flex items-center justify-center gap-2 transition-colors cursor-pointer rounded-xl"
              >
                <span class="material-symbols-outlined text-sm">lock_reset</span>
                <span>Change Password</span>
              </button>
            </form>
          </div>

          <!-- 2FA Card -->
          <div class="bg-surface-container border border-outline-variant p-xl rounded-xl">
            <div class="flex items-center justify-between mb-md">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-primary-fixed-dim">verified_user</span>
                <h3 class="font-title-sm text-on-surface font-bold">Two-Factor Auth</h3>
              </div>
              <span class="px-2 py-0.5 bg-surface-variant text-[10px] font-bold text-outline-variant rounded uppercase">
                Disabled
              </span>
            </div>
            <p class="text-sm text-on-surface-variant mb-lg">Add an extra layer of security using TOTP Authenticator apps.</p>
            <button
              type="button"
              class="w-full py-2 bg-primary-fixed-dim text-on-primary-fixed rounded text-sm font-bold flex items-center justify-center gap-2 hover:bg-primary-fixed-dim/90 transition-colors cursor-pointer rounded-xl"
            >
              <span class="material-symbols-outlined text-sm">qr_code_2</span>
              <span>Setup 2FA</span>
            </button>
          </div>
        </div>

        <!-- Right Column -->
        <div class="col-span-12 lg:col-span-9 flex flex-col gap-lg">
          <!-- Personal Information -->
          <div class="bg-surface-container border border-outline-variant p-xl rounded-xl">
            <div class="flex items-center justify-between mb-lg">
              <h3 class="font-title-sm text-on-surface font-bold">Personal Information</h3>
              <span v-if="savedNotice" class="text-xs font-bold text-tertiary-fixed-dim animate-pulse font-body-mono">
                ✓ Saved Changes!
              </span>
            </div>

            <div class="grid grid-cols-2 gap-xl">
              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  Full Name
                </label>
                <input
                  v-model="fullName"
                  type="text"
                  class="w-full bg-surface border border-outline-variant rounded px-md py-2 text-sm rounded-xl text-on-surface focus:ring-1 focus:ring-primary-fixed-dim"
                />
              </div>

              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  Department <span class="text-[10px] lowercase opacity-50">(readonly)</span>
                </label>
                <input
                  v-model="department"
                  type="text"
                  readonly
                  class="w-full bg-surface/50 border border-outline-variant rounded px-md py-2 text-sm text-outline-variant cursor-not-allowed rounded-xl"
                />
              </div>

              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  Role <span class="text-[10px] lowercase opacity-50">(readonly)</span>
                </label>
                <input
                  v-model="role"
                  type="text"
                  readonly
                  class="w-full bg-surface/50 border border-outline-variant rounded px-md py-2 text-sm text-outline-variant cursor-not-allowed rounded-xl"
                />
              </div>

              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  Email Address
                </label>
                <input
                  v-model="email"
                  type="email"
                  class="w-full bg-surface border border-outline-variant rounded px-md py-2 text-sm rounded-xl text-on-surface focus:ring-1 focus:ring-primary-fixed-dim"
                />
              </div>
            </div>

            <div class="flex justify-end mt-xl">
              <button
                type="button"
                class="bg-primary-fixed-dim text-on-primary-fixed px-lg py-2 rounded text-sm font-bold flex items-center gap-2 hover:bg-primary-fixed-dim/90 transition-colors rounded-xl cursor-pointer"
                @click="handleSaveProfile"
              >
                <span class="material-symbols-outlined text-sm">save</span>
                <span>Save Changes</span>
              </button>
            </div>
          </div>

          <!-- Appearance & Regionality -->
          <div class="bg-surface-container border border-outline-variant p-xl rounded-xl">
            <h3 class="font-title-sm text-on-surface mb-lg">Appearance &amp; Regionality</h3>
            <div class="grid grid-cols-2 gap-xl">
              <!-- Dynamic Theme Selector -->
              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  Theme
                </label>
                <select
                  :value="theme"
                  class="w-full bg-surface border border-outline-variant rounded px-md py-2 text-sm appearance-none rounded-xl text-on-surface cursor-pointer"
                  @change="handleThemeChange"
                >
                  <option value="dark">Dark</option>
                  <option value="light">Light</option>
                </select>
              </div>

              <!-- Language Selector -->
              <div>
                <label class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1 block">
                  Language
                </label>
                <select
                  :value="locale"
                  class="w-full bg-surface border border-outline-variant rounded px-md py-2 text-sm appearance-none rounded-xl text-on-surface cursor-pointer"
                  @change="handleLocaleChange"
                >
                  <option value="en">English (EN)</option>
                  <option value="ru">Russian (RU)</option>
                </select>
              </div>

              <!-- Timezone Selector -->
              <div class="col-span-2">
                <div class="flex items-center justify-between mb-1">
                  <label class="text-label-caps font-label-caps text-on-surface-variant uppercase block">
                    Timezone
                  </label>
                  <div
                    class="flex items-center gap-1 text-primary-fixed-dim text-xs cursor-pointer hover:underline"
                    @click="handleAutoDetectTimezone"
                  >
                    <span class="material-symbols-outlined text-sm">my_location</span>
                    <span>Auto-detect</span>
                  </div>
                </div>
                <div class="relative">
                  <select
                    v-model="timezone"
                    class="w-full bg-surface border border-outline-variant rounded px-md py-2 text-sm appearance-none rounded-xl text-on-surface cursor-pointer"
                  >
                    <option value="Europe/Minsk">Europe/Minsk</option>
                    <option value="Europe/Moscow">Europe/Moscow</option>
                    <option value="UTC">UTC</option>
                    <option value="America/New_York">America/New_York</option>
                  </select>
                  <span class="absolute right-3 top-1/2 -translate-y-1/2 material-symbols-outlined text-on-surface-variant text-sm pointer-events-none">
                    expand_more
                  </span>
                </div>
              </div>
            </div>
          </div>
          <!-- Notification Settings -->
          <div class="bg-surface-container border border-outline-variant p-xl rounded-xl">
            <div class="flex items-center justify-between mb-sm">
              <div>
                <h3 class="font-title-sm text-on-surface">{{ t('profile.notificationSettings') }}</h3>
                <p class="text-xs text-on-surface-variant">{{ t('profile.notificationDesc') }}</p>
              </div>
              <span class="material-symbols-outlined text-primary-fixed-dim">notifications</span>
            </div>

            <div class="flex flex-col gap-md">
              <!-- Do Not Disturb -->
              <div class="bg-surface border border-outline-variant/30 rounded p-md flex flex-col md:flex-row items-start md:items-center justify-between gap-md">
                <div class="flex items-center gap-md">
                  <span class="material-symbols-outlined text-outline-variant">do_not_disturb_on</span>
                  <div>
                    <p class="text-sm font-bold text-on-surface">{{ t('profile.doNotDisturb') }}</p>
                    <p class="text-xs text-on-surface-variant">{{ t('profile.doNotDisturbDesc') }}</p>
                  </div>
                </div>
                <div class="flex flex-wrap items-center gap-4">
                  <div class="flex flex-wrap items-center gap-2 text-xs text-on-surface-variant">
                    <span>Mute notifications:</span>
                    <button
                      v-for="dur in ['15 minutes', '1 hour', '8 hours', '24 hours']"
                      :key="dur"
                      type="button"
                      class="px-2 py-1 rounded text-on-surface rounded-xl transition-colors cursor-pointer"
                      :class="activeMuteDuration === dur ? 'bg-surface-variant font-bold' : 'hover:bg-surface-variant/50'"
                      @click="activeMuteDuration = dur"
                    >
                      {{ dur }}
                    </button>
                    <button
                      type="button"
                      class="px-2 py-1 hover:bg-surface-variant/50 rounded flex items-center gap-1 rounded-xl cursor-pointer"
                      :class="activeMuteDuration === 'inf' ? 'bg-surface-variant font-bold' : ''"
                      @click="activeMuteDuration = 'inf'"
                    >
                      <span class="material-symbols-outlined text-[10px]">pause</span>
                      <span>Until turned off</span>
                    </button>
                  </div>
                  <div class="flex items-center gap-2 bg-tertiary-fixed-dim/10 px-2 py-1 rounded">
                    <span class="w-2 h-2 rounded-full bg-tertiary-fixed-dim"></span>
                    <span class="text-[10px] font-bold text-tertiary-fixed-dim uppercase">{{ t('profile.notificationsActive') }}</span>
                  </div>
                </div>
              </div>

              <!-- Quiet Hours -->
              <div class="bg-surface border border-outline-variant/30 rounded p-md flex items-center justify-between">
                <div class="flex items-center gap-md">
                  <span class="material-symbols-outlined text-outline-variant">bedtime</span>
                  <div>
                    <p class="text-sm font-bold text-on-surface">{{ t('profile.quietHours') }}</p>
                    <p class="text-xs text-on-surface-variant">{{ t('profile.quietHoursDesc') }}</p>
                  </div>
                </div>
                <div
                  class="w-10 h-5 rounded-full relative cursor-pointer transition-colors"
                  :class="quietHoursEnabled ? 'bg-primary-fixed-dim' : 'bg-surface-variant'"
                  @click="quietHoursEnabled = !quietHoursEnabled"
                >
                  <div
                    class="absolute top-1 w-3 h-3 rounded-full transition-all"
                    :class="quietHoursEnabled ? 'left-6 bg-on-primary-fixed' : 'left-1 bg-outline-variant'"
                  ></div>
                </div>
              </div>

              <!-- Module Subscriptions -->
              <div class="mt-md">
                <p class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-sm">
                  {{ t('profile.moduleSubscriptions') }}
                </p>
                <p class="text-xs text-on-surface-variant mb-2">
                  {{ t('profile.moduleSubscriptionsDesc') }}
                </p>
                <div class="border border-outline-variant/30 rounded overflow-hidden">
                  <div class="flex flex-col md:flex-row items-start md:items-center justify-between p-md bg-surface gap-md">
                    <div class="flex items-center gap-md">
                      <input checked type="checkbox" class="rounded border-outline-variant bg-transparent text-primary-fixed-dim focus:ring-0 rounded-xl" />
                      <div>
                        <p class="text-sm font-bold text-on-surface">
                          {{ t('profile.systemCore') }}
                          <span class="text-[10px] font-mono text-outline-variant bg-surface-variant px-1 rounded">core</span>
                        </p>
                        <p class="text-xs text-on-surface-variant">{{ t('profile.systemCoreDesc') }}</p>
                      </div>
                    </div>
                    <div class="flex flex-wrap items-center gap-lg">
                      <div class="flex items-center gap-2 text-xs text-on-surface-variant">
                        <span>Module mute:</span>
                        <button type="button" class="px-2 py-1 bg-surface-variant rounded text-on-surface rounded-xl cursor-pointer">15m</button>
                        <button type="button" class="px-2 py-1 hover:bg-surface-variant/50 rounded rounded-xl cursor-pointer">1h</button>
                        <button type="button" class="px-2 py-1 hover:bg-surface-variant/50 rounded rounded-xl cursor-pointer">8h</button>
                        <button type="button" class="px-2 py-1 hover:bg-surface-variant/50 rounded flex items-center justify-center rounded-xl cursor-pointer">
                          <span class="material-symbols-outlined text-[10px]">all_inclusive</span>
                        </button>
                      </div>
                      <div class="flex items-center gap-2">
                        <span class="text-[10px] text-outline-variant uppercase">Module sound:</span>
                        <select class="bg-surface-dim border border-outline-variant/50 rounded px-2 py-1 text-xs text-on-surface rounded-xl">
                          <option>Default (by type)</option>
                        </select>
                        <button type="button" class="text-primary-fixed-dim rounded-xl cursor-pointer">
                          <span class="material-symbols-outlined text-sm">play_arrow</span>
                        </button>
                      </div>
                      <div class="flex items-center gap-2">
                        <span class="text-[10px] text-outline-variant uppercase">Severity threshold:</span>
                        <select class="bg-surface-dim border border-outline-variant/50 rounded px-2 py-1 text-xs text-on-surface rounded-xl">
                          <option>All events (Info+)</option>
                        </select>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Notification Sound Signals -->
              <div class="mt-md">
                <div class="flex items-center gap-2 mb-sm">
                  <span class="material-symbols-outlined text-primary-fixed-dim">volume_up</span>
                  <p class="text-label-caps font-label-caps text-on-surface-variant uppercase">
                    {{ t('profile.soundSignals') }}
                  </p>
                </div>
                <p class="text-xs text-on-surface-variant mb-2">{{ t('profile.soundSignalsDesc') }}</p>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <!-- Info -->
                  <div class="flex items-center justify-between p-3 border border-outline-variant/30 rounded bg-surface-variant">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-primary-fixed-dim">info</span>
                      <span class="text-sm font-bold text-on-surface">{{ t('profile.infoSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <select class="bg-surface-dim border border-outline-variant/50 rounded px-2 py-1 text-xs text-on-surface w-32 rounded-xl">
                        <option>Soft Chime</option>
                      </select>
                      <button type="button" class="text-primary-fixed-dim rounded-xl cursor-pointer">
                        <span class="material-symbols-outlined text-sm">play_arrow</span>
                      </button>
                    </div>
                  </div>

                  <!-- Success -->
                  <div class="flex items-center justify-between p-3 border border-outline-variant/30 rounded-xl bg-surface-container">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-tertiary-fixed-dim">check_circle</span>
                      <span class="text-sm font-bold text-on-surface">{{ t('profile.successSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <select class="bg-surface-dim border border-outline-variant/50 rounded px-2 py-1 text-xs text-on-surface w-32 rounded-xl">
                        <option>Major Chord</option>
                      </select>
                      <button type="button" class="text-primary-fixed-dim rounded-xl cursor-pointer">
                        <span class="material-symbols-outlined text-sm">play_arrow</span>
                      </button>
                    </div>
                  </div>

                  <!-- Warning -->
                  <div class="flex items-center justify-between p-3 border border-outline-variant/30 rounded-xl bg-surface-container">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-warning-yellow">warning</span>
                      <span class="text-sm font-bold text-on-surface">{{ t('profile.warningSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <select class="bg-surface-dim border border-outline-variant/50 rounded px-2 py-1 text-xs text-on-surface w-32 rounded-xl">
                        <option>Double Beep</option>
                      </select>
                      <button type="button" class="text-primary-fixed-dim rounded-xl cursor-pointer">
                        <span class="material-symbols-outlined text-sm">play_arrow</span>
                      </button>
                    </div>
                  </div>

                  <!-- Error -->
                  <div class="flex items-center justify-between p-3 border border-outline-variant/30 rounded-xl bg-surface-container">
                    <div class="flex items-center gap-2">
                      <span class="material-symbols-outlined text-error">error</span>
                      <span class="text-sm font-bold text-on-surface">{{ t('profile.errorSeverity') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <select class="bg-surface-dim border border-outline-variant/50 rounded px-2 py-1 text-xs text-on-surface w-32 rounded-xl">
                        <option>Alarm Tone</option>
                      </select>
                      <button type="button" class="text-primary-fixed-dim rounded-xl cursor-pointer">
                        <span class="material-symbols-outlined text-sm">play_arrow</span>
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Active Sessions -->
          <div class="bg-surface-container border border-outline-variant p-xl rounded-xl">
            <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-md mb-lg">
              <div>
                <h3 class="font-title-sm text-on-surface">{{ t('profile.activeSessions') }}</h3>
                <p class="text-xs text-on-surface-variant">{{ t('profile.activeSessionsDesc') }}</p>
              </div>
              <div class="flex gap-2">
                <button
                  type="button"
                  class="px-3 py-1.5 border border-tertiary-fixed-dim text-tertiary-fixed-dim text-[10px] font-bold uppercase rounded hover:bg-tertiary-fixed-dim/10 transition-colors flex items-center gap-1 rounded-xl cursor-pointer"
                >
                  <span class="material-symbols-outlined text-sm">cancel</span>
                  <span>{{ t('profile.terminateOthers') }}</span>
                </button>
                <button
                  type="button"
                  class="px-3 py-1.5 border border-outline-variant text-on-surface text-[10px] font-bold uppercase rounded hover:bg-surface-variant transition-colors flex items-center gap-1 rounded-xl cursor-pointer"
                >
                  <span class="material-symbols-outlined text-sm">logout</span>
                  <span>{{ t('profile.allLogout') }}</span>
                </button>
              </div>
            </div>

            <div class="overflow-x-auto">
              <table class="w-full text-left border-collapse">
                <thead>
                  <tr class="border-b border-outline-variant/30">
                    <th class="py-2 text-label-caps font-label-caps text-on-surface-variant uppercase">{{ t('profile.ipAddress') }}</th>
                    <th class="py-2 text-label-caps font-label-caps text-on-surface-variant uppercase">{{ t('profile.deviceBrowser') }}</th>
                    <th class="py-2 text-label-caps font-label-caps text-on-surface-variant uppercase">{{ t('profile.lastSeen') }}</th>
                    <th class="py-2 text-label-caps font-label-caps text-on-surface-variant uppercase text-right">{{ t('common.actions') }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr class="border-b border-outline-variant/10">
                    <td class="py-4">
                      <div class="flex items-center gap-2">
                        <span class="text-sm font-body-mono text-primary-fixed-dim">127.0.0.1</span>
                        <span class="px-1.5 py-0.5 bg-tertiary-fixed-dim/20 text-tertiary-fixed-dim text-[8px] font-bold rounded uppercase">
                          {{ t('profile.currentSession') }}
                        </span>
                      </div>
                    </td>
                    <td class="py-4 text-sm text-on-surface-variant">Edge (Windows)</td>
                    <td class="py-4 text-sm text-on-surface-variant font-body-mono">{{ new Date().toLocaleString() }}</td>
                    <td class="py-4 text-right">
                      <button
                        type="button"
                        class="px-3 py-1 bg-surface border border-outline-variant hover:bg-surface-variant/80 text-on-surface text-[10px] font-bold uppercase rounded transition-colors rounded-xl cursor-pointer"
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
    </main>
  </div>
</template>
