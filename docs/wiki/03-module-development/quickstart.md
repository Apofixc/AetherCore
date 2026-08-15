# 🚀 Руководство разработчика модулей (.nms-plugin)

## 1. Анатомия пакета плагина

Каждый модуль поставляется в виде единого ZIP-архива с расширением `.nms-plugin`:

```text
my-plugin-1.0.0.nms-plugin (ZIP)
├── manifest.yaml             # Декларативный контракт модуля
├── backend.wasm              # Байткод бэкенда (WASM Component Model)
├── signature.bin             # Цифровая подпись Ed25519 (опционально)
├── locales/                  # Словари локализации
│   ├── ru.json
│   └── en.json
└── frontend/                 # Пользовательский интерфейс
    ├── dist/ui.js            # Скомпилированный ESM бандл (Level 3)
    └── views/                # Исходные Vue SFC компоненты (Level 2)
```

---

## 2. Пошаговое создание нового модуля

### Шаг 1. Создание манифеста `manifest.yaml`
Определите идентификатор, версию, маршруты UI и схему настроек:
```yaml
manifest_version: 1
id: "my-service"
name: "Мой Сервис"
version: "1.0.0"
description: "Сервис обработки данных"
type: "feature"
enabled_by_default: true
min_core_version: "2.0.0"

events:
  publishes:
    - "my-service.event_created"
  subscribes:
    - "core.system_started"

routes:
  - path: "/my-service"
    name: "my-service-dashboard"
    component: "frontend/dist/ui.js#MyServiceView"
    meta:
      title: "Мой Сервис"
      icon: "cpu"
      group: "general"
```

### Шаг 2. Создание словарей локализации
В каталоге `locales/ru.json`:
```json
{
  "title": "Мой Сервис",
  "status.ok": "Сервис работает стабильно"
}
```

В каталоге `locales/en.json`:
```json
{
  "title": "My Service",
  "status.ok": "Service is running stably"
}
```

### Шаг 3. Создание UI компонента (`frontend/dist/ui.js`)
```javascript
export const MyServiceView = {
  name: 'MyServiceView',
  template: `
    <div class="p-6 bg-white dark:bg-slate-900 rounded-xl shadow-md">
      <h2 class="text-2xl font-bold mb-4">{{ $t('my-service.title') }}</h2>
      <p class="text-emerald-600 font-semibold">{{ $t('my-service.status.ok') }}</p>
    </div>
  `
};

export default { MyServiceView };
```

---

## 3. Сборка и упаковка плагина через CLI `nms`

Для упаковки папки плагина в архив `.nms-plugin` выполните:
```bash
cargo run -p nms-cli -- plugin pack plugins/my-service --output modules/my-service-1.0.0.nms-plugin
```

После этого поместите файл `.nms-plugin` в директорию `modules/` сервера. Микроядро автоматически подгрузит его без перезапуска!
