# 🚀 Руководство по развертыванию и конфигурации

## 1. Режимы запуска бинарного файла `nms`

Исполняемый файл платформы `nms` поддерживает следующие режимы работы и аргументы CLI:

```bash
# 1. Запуск в режиме Headless HTTP/WS сервера на порту 8080
./target/release/nms --server --port 8080 --host 0.0.0.0

# 2. Запуск в режиме локальной разработки (разрешение неподписанных плагинов)
./target/release/nms --server --dev

# 3. Аварийный режим Safe-Mode (старт ядра без загрузки сторонних плагинов)
./target/release/nms --server --safe-mode

# 4. Указание кастомного пути к базе данных и каталогу плагинов
./target/release/nms --server --db /var/lib/nms/data.db --modules-dir /var/lib/nms/modules
```

---

## 2. Параметры командной строки (CLI Flags)

| Флаг | Тип / Дефолт | Описание |
| :--- | :--- | :--- |
| `--server` | `flag` | Запуск приложения в качестве headless сетевого демона (без GUI) |
| `--host` | `string` (`127.0.0.1`) | IP-адрес интерфейса для привязки TCP-сокета HTTP/WS сервера |
| `--port` | `u16` (`8080`) | Порт HTTP/WS сервера |
| `--dev` | `flag` (`false`) | Режим разработки: разрешение загрузки неподписанных `.nms-plugin` пакетов |
| `--safe-mode` | `flag` (`false`) | Аварийный режим: старт ядра без загрузки пользовательских плагинов |
| `--db` | `path` (`data/nms.db`) | Путь к файлу базы данных SQLite (WAL) |
| `--modules-dir`| `path` (`modules`) | Путь к каталогу установленных плагинов |

---

## 3. Конфигурация приложения (`config.toml`)

Конфигурация может задаваться файлом настроек `config.toml` в корне или рабочей директории:

```toml
[server]
host = "0.0.0.0"
port = 8080
dev_mode = false
safe_mode = false

[database]
path = "data/nms.db"
max_read_connections = 10
busy_timeout_ms = 5000

[security]
jwt_secret = "your-cryptographically-secure-random-secret-key-32-bytes"
jwt_ttl_seconds = 86400           # Время жизни JWT токена (24 часа)
allow_unsigned_plugins = false
trusted_public_keys = [
    # Список Base64 публичных Ed25519 ключей доверенных издателей
]

[plugins]
dir = "modules"
cache_dir = "cache/modules"
memory_limit_mb = 128             # Лимит памяти песочницы на плагин
execution_timeout_sec = 5         # Таймаут прерывания гостевого кода (Epoch deadline)

[i18n]
default_locale = "ru"             # Язык по умолчанию ("ru" или "en")
```

---

## 4. Развертывание через systemd (Linux)

Создайте service-файл `/etc/systemd/system/nms.service`:

```ini
[Unit]
Description=Next-Gen Universal Core Platform Daemon
After=network.target

[Service]
Type=simple
User=nms
Group=nms
WorkingDirectory=/opt/nms
ExecStart=/opt/nms/bin/nms --server --host 0.0.0.0 --port 8080
Restart=always
RestartSec=5s
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

Активация и запуск:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now nms
sudo systemctl status nms
```
