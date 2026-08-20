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

router.beforeEach((to: RouteLocationNormalized, _from: RouteLocationNormalized, next: NavigationGuardNext) => {
  const token = localStorage.getItem('nms_token')
  if (!to.meta.public && !token) {
    next('/login')
  } else if (to.path === '/login' && token) {
    next('/dashboard')
  } else {
    next()
  }
})
