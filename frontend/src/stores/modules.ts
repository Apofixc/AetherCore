import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { modulesApi, type ModuleDto } from '@/api/modules'

export const useModulesStore = defineStore('modules', () => {
  const modules = ref<ModuleDto[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const selectedModule = ref<ModuleDto | null>(null)
  const filter = ref<'all' | 'active' | 'disabled'>('all')

  const totalCount = computed(() => modules.value.length)
  const activeCount = computed(() => modules.value.filter(m => m.is_active).length)
  const disabledCount = computed(() => modules.value.filter(m => !m.is_active).length)

  const filteredModules = computed(() => {
    if (filter.value === 'active') return modules.value.filter(m => m.is_active)
    if (filter.value === 'disabled') return modules.value.filter(m => !m.is_active)
    return modules.value
  })

  async function fetchModules() {
    loading.value = true
    error.value = null
    try {
      const list = await modulesApi.list()
      modules.value = list
    } catch (err: any) {
      error.value = err.message || 'Failed to fetch modules'
    } finally {
      loading.value = false
    }
  }

  async function toggleModule(id: string, enable: boolean) {
    loading.value = true
    try {
      if (enable) {
        await modulesApi.enable(id)
      } else {
        await modulesApi.disable(id)
      }
      const mod = modules.value.find(m => m.id === id)
      if (mod) {
        mod.is_active = enable
      }
      if (selectedModule.value?.id === id) {
        selectedModule.value.is_active = enable
      }
    } catch (err: any) {
      error.value = err.message
      throw err
    } finally {
      loading.value = false
    }
  }

  function selectModule(mod: ModuleDto | null) {
    selectedModule.value = mod
  }

  function setFilter(newFilter: 'all' | 'active' | 'disabled') {
    filter.value = newFilter
  }

  return {
    modules,
    loading,
    error,
    selectedModule,
    filter,
    totalCount,
    activeCount,
    disabledCount,
    filteredModules,
    fetchModules,
    toggleModule,
    selectModule,
    setFilter
  }
})
