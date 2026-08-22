import { createRouter, createWebHistory, type RouteLocationNormalized, type NavigationGuardNext } from 'vue-router'
import LoginView from '@/views/LoginView.vue'
import DashboardView from '@/views/DashboardView.vue'
import ModuleManagementView from '@/views/ModuleManagementView.vue'
import UserProfileView from '@/views/UserProfileView.vue'
import UsersManagementView from '@/views/UsersManagementView.vue'
import AccessIdentityView from '@/views/AccessIdentityView.vue'
import SystemAdminView from '@/views/SystemAdminView.vue'

const routes = [
  {
    path: '/login',
    name: 'login',
    component: LoginView,
    meta: { public: true }
  },
  {
    path: '/',
    redirect: '/dashboard'
  },
  {
    path: '/dashboard',
    name: 'dashboard',
    component: DashboardView
  },
  {
    path: '/modules',
    redirect: '/settings/modules'
  },
  {
    path: '/settings/modules',
    name: 'modules',
    component: ModuleManagementView
  },
  {
    path: '/profile',
    redirect: '/settings/profile'
  },
  {
    path: '/settings/profile',
    name: 'profile',
    component: UserProfileView
  },
  {
    path: '/users',
    redirect: '/settings/users'
  },
  {
    path: '/settings/users',
    name: 'users',
    component: UsersManagementView
  },
  {
    path: '/settings',
    redirect: '/settings/modules'
  },
  {
    path: '/settings/access',
    redirect: '/settings/access-identity'
  },
  {
    path: '/settings/access-identity',
    name: 'access-identity',
    component: AccessIdentityView
  },
  {
    path: '/system',
    redirect: '/settings/system'
  },
  {
    path: '/settings/system',
    name: 'system-admin',
    component: SystemAdminView
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: '/dashboard'
  }
]

export const router = createRouter({
  history: createWebHistory(),
  routes
})

import { useAuthStore } from '@/stores/auth'

router.beforeEach(async (to: RouteLocationNormalized, _from: RouteLocationNormalized, next: NavigationGuardNext) => {
  const authStore = useAuthStore()
  if (!authStore.authConfig) {
    await authStore.checkAuthConfig()
  }

  const isAuthDisabled = authStore.authConfig?.web_ui_auth === false
  const token = localStorage.getItem('aether_token') || sessionStorage.getItem('aether_token')

  if (isAuthDisabled) {
    if (to.path === '/login') {
      next('/dashboard')
    } else {
      next()
    }
    return
  }

  if (!to.meta.public && !token) {
    next('/login')
    return
  }

  if (token) {
    if (!authStore.user) {
      await authStore.fetchUser()
    }

    // Проверка политики обязательного 2FA (Enforced 2FA Policy)
    const isEnforced2fa = Boolean(authStore.authConfig?.force_2fa && authStore.user && !authStore.user.is_totp_enabled)
    if (isEnforced2fa) {
      if (to.path !== '/settings/profile') {
        next('/settings/profile?setup_2fa=true')
        return
      }
    }

    if (to.path === '/login') {
      if (isEnforced2fa) {
        next('/settings/profile?setup_2fa=true')
      } else {
        next('/dashboard')
      }
      return
    }
  }

  next()
})
