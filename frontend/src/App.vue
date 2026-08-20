<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import AppHeader from '@/components/layout/AppHeader.vue'
import AppFooter from '@/components/layout/AppFooter.vue'
import { useAuthStore } from '@/stores/auth'

const route = useRoute()
const authStore = useAuthStore()
const sidebarCollapsed = ref(false)

const isPublicRoute = computed(() => route.meta.public === true)

onMounted(async () => {
  if (authStore.isAuthenticated) {
    await authStore.fetchUser()
  }
})
</script>

<template>
  <!-- Public Layout (Login Screen) -->
  <div v-if="isPublicRoute" class="min-h-screen bg-background text-on-surface">
    <router-view />
  </div>

  <!-- Authenticated Shell Layout -->
  <div
    v-else
    class="flex bg-background h-screen w-screen text-on-surface overflow-hidden relative"
    :class="{ 'sidebar-collapsed': sidebarCollapsed }"
  >
    <!-- Sidebar -->
    <AppSidebar :collapsed="sidebarCollapsed" @toggle="sidebarCollapsed = !sidebarCollapsed" />

    <!-- Main Content Wrapper -->
    <div
      id="main-content"
      class="ml-sidebar-width flex-1 flex flex-col w-[calc(100vw-312px)] transition-all duration-300 h-screen overflow-hidden"
      :class="sidebarCollapsed ? '!ml-0 !w-full' : ''"
    >
      <!-- Top App Bar -->
      <AppHeader :sidebar-collapsed="sidebarCollapsed" @toggle-sidebar="sidebarCollapsed = !sidebarCollapsed" />

      <!-- View Canvas -->
      <router-view />
    </div>

    <!-- Bottom Status Bar / Footer -->
    <AppFooter :class="sidebarCollapsed ? '!ml-0 !w-full' : ''" />
  </div>
</template>
