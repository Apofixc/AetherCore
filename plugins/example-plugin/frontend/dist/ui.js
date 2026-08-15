// ESM бандл интерфейса для example-plugin (Dual-mode mainstream)
export const DemoDashboardView = {
  name: 'DemoDashboardView',
  template: `
    <div class="p-6 bg-white dark:bg-slate-900 rounded-xl shadow-md border border-slate-200 dark:border-slate-800">
      <h2 class="text-2xl font-bold text-slate-800 dark:text-slate-100 mb-4">{{ $t('example-plugin.title') }}</h2>
      <p class="text-slate-600 dark:text-slate-300">{{ $t('example-plugin.status.active') }}</p>
    </div>
  `
};

export const DemoSummaryWidget = {
  name: 'DemoSummaryWidget',
  template: `
    <div class="p-4 bg-slate-50 dark:bg-slate-800 rounded-lg">
      <h3 class="font-semibold text-sm">{{ $t('example-plugin.widget.summary_title') }}</h3>
      <div class="mt-2 flex items-center gap-2">
        <span class="inline-block w-2.5 h-2.5 bg-emerald-500 rounded-full animate-pulse"></span>
        <span class="text-xs text-slate-500 dark:text-slate-400">Online</span>
      </div>
    </div>
  `
};

export default {
  DemoDashboardView,
  DemoSummaryWidget
};
