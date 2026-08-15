# Манифест плагина (`manifest.yaml`)

Единый декларативный контракт модуля. Пакет плагина — единый zip-архив `.nms-plugin`
(Zero-Unpack: ядро читает `manifest.yaml` и `backend.wasm` напрямую из архива в память),
либо распакованный dev-каталог в `modules/`.

## Структура пакета

```text
my-module.nms-plugin (zip)
├── manifest.yaml      # обязателен
├── backend.wasm       # WASM-компонент (опционален для UI-only модулей)
└── signature.bin      # Ed25519-подпись (manifest.yaml + backend.wasm)
```

## Пример манифеста

```yaml
manifest_version: 1
id: net-scanner            # kebab-case, неймспейс шины/KV/API
name: Network Scanner
version: 1.2.0             # SemVer
type: feature              # system | feature | driver
min_core_version: 0.1.0
max_core_version: 1.0.0

deps: []                   # обязательные зависимости (DAG-порядок загрузки)
optional_deps: [dashboard] # мягкая деградация при отсутствии

capabilities:
  network:
    allowed_hosts: ["192.168.0.0/16"]
  filesystem:
    allow_host_dirs:
      - { path: /var/lib/nms/scans, mode: read_write }
  environment:
    allow_env_vars: [SNMP_COMMUNITY]

events:
  publishes:               # обязаны начинаться с "{id}."
    - net-scanner.device.up
    - net-scanner.device.down
  subscribes:
    - core.system.startup

routes:
  - path: /net-scanner
    name: net-scanner-main
    component: dist/ui.js  # ESM | views/*.vue (dev) | null (Schema-Driven)
    meta: { title: Scanner, requires_auth: true }

permissions:
  - { id: net-scanner.view, name: View scan results }

config_schema:             # JSON Schema: валидация + автогенерация формы
  type: object
  required: [interval]
  properties:
    interval: { type: integer, minimum: 1 }

hooks:
  on_enable: on-enable
  on_disable: on-disable
```

## Валидация при загрузке

`ModuleManifest::validate` блокирует модуль при:

- некорректном `id` (не kebab-case) или не-SemVer `version`;
- несовместимости версии ядра с `min_core_version`/`max_core_version`;
- спуфинге топиков: публикуемый топик без префикса `{id}.`;
- зависимости модуля от самого себя;
- синтаксически некорректной `config_schema`.

## Политика подписей

Подпись Ed25519 покрывает конкатенацию `manifest.yaml + backend.wasm` и проверяется
по доверенным ключам `PluginEngine::trusted_keys`:

- невалидная подпись — модуль блокируется всегда;
- отсутствие подписи — модуль блокируется при `allow_unsigned_plugins = false` (продакшн).

## Порядок загрузки

Зависимости строят DAG; `toposort` гарантирует загрузку провайдеров раньше потребителей,
детерминированный порядок и обнаружение циклов. Отсутствующие `optional_deps`
фиксируются в `TopoResult::missing_optional` без блокировки модуля.
