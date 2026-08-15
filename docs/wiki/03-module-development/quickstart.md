# Руководство по разработке модулей (.nms-plugin)

## Структура плагина
Плагин поставляется в виде единого ZIP-архива с расширением `.nms-plugin`:

```text
my-plugin-1.0.0.nms-plugin (ZIP)
├── manifest.yaml           # Манифест модуля (контракт)
├── backend.wasm            # Байткод бэкенда (опционально)
├── signature.bin           # Ed25519 цифровая подпись (опционально)
├── locales/                # Словари локализации (ru.json, en.json)
└── frontend/               # Фронтенд ассеты (dist/ui.js, views/)
```

## Сборка и упаковка плагина
Собрать и упаковать плагин в единый архив можно с помощью CLI утилиты `nms`:
```bash
cargo run -p nms-cli -- plugin pack plugins/my-plugin --output modules/my-plugin-1.0.0.nms-plugin
```
