#!/usr/bin/env bash
# ==============================================================================
# Скрипт запуска AetherCore Platform (Бэкенд Rust Axum + Фронтенд Vue 3 Vite Dev)
# ==============================================================================

set -eo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

# Цвета для терминала
CLR_RESET="\033[0m"
CLR_BOLD="\033[1m"
CLR_RED="\033[31m"
CLR_GREEN="\033[32m"
CLR_YELLOW="\033[33m"
CLR_BLUE="\033[34m"
CLR_CYAN="\033[36m"

# Конфигурация портов и хоста по умолчанию
BACKEND_PORT="${BACKEND_PORT:-3000}"
FRONTEND_PORT="${FRONTEND_PORT:-5173}"
HOST="${HOST:-0.0.0.0}"
BACKEND_HOST="127.0.0.1"

# Переменные состояния процессов
BACKEND_PID=""
FRONTEND_PID=""
SAFE_MODE=false
NO_WATCH=false
PROD_MODE=false

show_help() {
    echo -e "${CLR_BOLD}Использование:${CLR_RESET} ./start.sh [КОМАНДА] [ОПЦИИ]"
    echo ""
    echo -e "${CLR_BOLD}Команды:${CLR_RESET}"
    echo "  all             Запуск Backend и Frontend в dev-режиме (по умолчанию)"
    echo "  frontend, -f    Запуск только веб-интерфейса (Vite Dev Server)"
    echo "  backend, -b     Запуск только бэкенда (Rust Axum)"
    echo "  prod            Сборка и запуск в production-режиме"
    echo "  check           Проверка готовности окружения и зависимостей"
    echo "  build           Сборка фронтенда и бэкенда"
    echo "  help, -h, --help Справка по использованию"
    echo ""
    echo -e "${CLR_BOLD}Опции:${CLR_RESET}"
    echo "  --safe-mode     Запуск ядра бэкенда в безопасном режиме (без сторонних плагинов)"
    echo "  --no-watch      Запуск бэкенда напрямую через cargo run (без cargo-watch)"
    echo ""
    echo -e "${CLR_BOLD}Переменные окружения:${CLR_RESET}"
    echo "  BACKEND_PORT    Порт API сервера (по умолчанию: 3000)"
    echo "  FRONTEND_PORT   Порт Vite Dev сервера (по умолчанию: 5173)"
    echo "  HOST            Хост для привязки фронтенда (по умолчанию: 0.0.0.0)"
    exit 0
}

# 1. Проверка системного окружения (Pre-flight checks)
check_prerequisites() {
    local missing=0

    echo -e "${CLR_CYAN}🔍 Проверка зависимостей и окружения...${CLR_RESET}"

    if ! command -v cargo >/dev/null 2>&1; then
        echo -e "${CLR_RED}❌ Ошибка: Rust/Cargo не найден в PATH. Установите rustup (https://rustup.rs).${CLR_RESET}"
        missing=1
    else
        echo -e "${CLR_GREEN}✓ Rust/Cargo доступен ($(cargo --version | head -n 1))${CLR_RESET}"
    fi

    if ! command -v node >/dev/null 2>&1; then
        echo -e "${CLR_RED}❌ Ошибка: Node.js не найден в PATH.${CLR_RESET}"
        missing=1
    else
        echo -e "${CLR_GREEN}✓ Node.js доступен ($(node --version))${CLR_RESET}"
    fi

    if ! command -v npm >/dev/null 2>&1; then
        echo -e "${CLR_RED}❌ Ошибка: npm не найден в PATH.${CLR_RESET}"
        missing=1
    else
        echo -e "${CLR_GREEN}✓ npm доступен ($(npm --version))${CLR_RESET}"
    fi

    # Создание необходимых рабочих директорий
    mkdir -p data modules cache

    if [ $missing -ne 0 ]; then
        echo -e "${CLR_RED}❌ Требуемые утилиты отсутствуют. Запуск прерван.${CLR_RESET}"
        exit 1
    fi
}

# 2. Безопасное освобождение портов (без pkill node)
free_port() {
    local port=$1
    if command -v fuser >/dev/null 2>&1; then
        fuser -k "${port}/tcp" 2>/dev/null || true
    elif command -v lsof >/dev/null 2>&1; then
        local pids
        pids=$(lsof -ti :"${port}" 2>/dev/null || true)
        if [ -n "$pids" ]; then
            kill -TERM $pids 2>/dev/null || true
            sleep 0.5
            kill -KILL $pids 2>/dev/null || true
        fi
    fi
}

# 3. Безопасное завершение процессов (только дочерние PID)
cleanup() {
    trap - INT TERM EXIT
    echo ""
    echo -e "${CLR_YELLOW}🛑 Завершение сервисов AetherCore...${CLR_RESET}"

    # Мягкая остановка Frontend
    if [ -n "$FRONTEND_PID" ] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
        kill -TERM "$FRONTEND_PID" 2>/dev/null || true
    fi

    # Мягкая остановка Backend
    if [ -n "$BACKEND_PID" ] && kill -0 "$BACKEND_PID" 2>/dev/null; then
        kill -TERM "$BACKEND_PID" 2>/dev/null || true
    fi

    # Ожидание завершения до 3 секунд
    local count=0
    while [ $count -lt 6 ]; do
        local running=0
        if [ -n "$FRONTEND_PID" ] && kill -0 "$FRONTEND_PID" 2>/dev/null; then running=1; fi
        if [ -n "$BACKEND_PID" ] && kill -0 "$BACKEND_PID" 2>/dev/null; then running=1; fi
        if [ $running -eq 0 ]; then break; fi
        sleep 0.5
        count=$((count + 1))
    done

    # Принудительная остановка при зависании
    if [ -n "$FRONTEND_PID" ] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
        kill -KILL "$FRONTEND_PID" 2>/dev/null || true
    fi
    if [ -n "$BACKEND_PID" ] && kill -0 "$BACKEND_PID" 2>/dev/null; then
        kill -KILL "$BACKEND_PID" 2>/dev/null || true
    fi

    echo -e "${CLR_GREEN}✓ Все процессы успешно остановлены.${CLR_RESET}"
    exit 0
}

trap cleanup INT TERM EXIT

# 4. Проверка готовности бэкенда (Healthcheck)
wait_for_backend() {
    local host=$1
    local port=$2
    local retries=30
    echo -ne "${CLR_CYAN}⏳ Ожидание инициализации REST API (${host}:${port})...${CLR_RESET}"

    for ((i=1; i<=retries; i++)); do
        if command -v curl >/dev/null 2>&1; then
            if curl -s -m 1 "http://${host}:${port}/api/health" >/dev/null 2>&1 || \
               curl -s -m 1 "http://${host}:${port}/" >/dev/null 2>&1; then
                echo -e " ${CLR_GREEN}Готов!${CLR_RESET}"
                return 0
            fi
        elif (echo > /dev/tcp/"${host}"/"${port}") >/dev/null 2>&1; then
            echo -e " ${CLR_GREEN}Готов!${CLR_RESET}"
            return 0
        fi
        sleep 0.5
    done

    echo -e " ${CLR_YELLOW}(Сервер запускается, переходим к следующему шагу)${CLR_RESET}"
    return 0
}

# 5. Запуск Frontend
start_frontend() {
    if [ ! -d "frontend" ]; then
        echo -e "${CLR_YELLOW}⚠️ Директория frontend не найдена. Запуск frontend пропущен.${CLR_RESET}"
        return 0
    fi

    echo -e "${CLR_CYAN}📦 Проверка зависимостей frontend...${CLR_RESET}"
    if [ ! -d "frontend/node_modules" ] || [ "frontend/package.json" -nt "frontend/node_modules" ]; then
        npm --prefix frontend install
    fi

    free_port "$FRONTEND_PORT"

    if [ "$PROD_MODE" = true ]; then
        echo -e "${CLR_GREEN}🎨 Запуск Frontend Preview сервера на http://localhost:${FRONTEND_PORT}...${CLR_RESET}"
        npm --prefix frontend run preview -- --host "${HOST}" --port "${FRONTEND_PORT}" &
    else
        echo -e "${CLR_GREEN}🎨 Запуск Frontend Vite Dev сервера на http://localhost:${FRONTEND_PORT}...${CLR_RESET}"
        npm --prefix frontend run dev -- --host "${HOST}" --port "${FRONTEND_PORT}" &
    fi
    FRONTEND_PID=$!
}

# 6. Запуск Backend
start_backend() {
    free_port "$BACKEND_PORT"

    local extra_args=""
    if [ "$SAFE_MODE" = true ]; then
        extra_args="--safe-mode"
        echo -e "${CLR_YELLOW}🛡️  Режим Safe-Mode активирован (сторонние плагины отключены)${CLR_RESET}"
    fi

    if [ "$PROD_MODE" = true ]; then
        echo -e "${CLR_GREEN}⚙️  Запуск Rust бэкенда в Release-режиме на http://${BACKEND_HOST}:${BACKEND_PORT}...${CLR_RESET}"
        cargo run --release -p aethercore-cli -- --server --host "${BACKEND_HOST}" --port "${BACKEND_PORT}" ${extra_args} &
    else
        echo -e "${CLR_GREEN}⚙️  Запуск Rust бэкенда на http://${BACKEND_HOST}:${BACKEND_PORT} (режим --dev)...${CLR_RESET}"
        if [ "$NO_WATCH" = false ] && command -v cargo-watch >/dev/null 2>&1; then
            cargo watch -q -c -w crates -x "run -p aethercore-cli -- --server --host ${BACKEND_HOST} --port ${BACKEND_PORT} --dev ${extra_args}" &
        else
            cargo run -p aethercore-cli -- --server --host "${BACKEND_HOST}" --port "${BACKEND_PORT}" --dev ${extra_args} &
        fi
    fi
    BACKEND_PID=$!
}

# Разбор аргументов командной строки
COMMAND="all"

while [[ $# -gt 0 ]]; do
    case "$1" in
        all|frontend|-f|--frontend|backend|-b|--backend|check|build|prod|help|-h|--help)
            COMMAND="$1"
            shift
            ;;
        --safe-mode)
            SAFE_MODE=true
            shift
            ;;
        --no-watch)
            NO_WATCH=true
            shift
            ;;
        *)
            echo -e "${CLR_RED}Неизвестный параметр: $1${CLR_RESET}"
            show_help
            ;;
    esac
done

case "$COMMAND" in
    help|-h|--help)
        show_help
        ;;
    check)
        check_prerequisites
        echo -e "${CLR_GREEN}✅ Все системные проверки пройдены успешно!${CLR_RESET}"
        exit 0
        ;;
    build)
        check_prerequisites
        echo -e "${CLR_CYAN}🔨 Сборка Rust бэкенда (--release)...${CLR_RESET}"
        cargo build --release -p aethercore-cli
        echo -e "${CLR_CYAN}🔨 Сборка Frontend бандла...${CLR_RESET}"
        npm --prefix frontend install
        npm --prefix frontend run build
        echo -e "${CLR_GREEN}✅ Проект успешно собран!${CLR_RESET}"
        exit 0
        ;;
    prod)
        PROD_MODE=true
        check_prerequisites
        start_backend
        wait_for_backend "$BACKEND_HOST" "$BACKEND_PORT"
        start_frontend
        echo ""
        echo "========================================================"
        echo -e "${CLR_GREEN}  ✨ AetherCore Platform запущен в Production режиме!${CLR_RESET}"
        echo "  🌐 Веб-интерфейс: http://localhost:${FRONTEND_PORT}"
        echo "  📡 REST API / WS: http://${BACKEND_HOST}:${BACKEND_PORT}"
        echo "  Для остановки нажмите Ctrl + C"
        echo "========================================================"
        echo ""
        wait -n "$BACKEND_PID" "$FRONTEND_PID" 2>/dev/null || wait
        ;;
    frontend|-f|--frontend)
        check_prerequisites
        start_frontend
        echo ""
        echo -e "${CLR_GREEN}✨ Запущен только Frontend в режиме разработки (HMR включен)!${CLR_RESET}"
        echo "🌐 Веб-интерфейс: http://localhost:${FRONTEND_PORT}"
        wait "$FRONTEND_PID" 2>/dev/null || wait
        ;;
    backend|-b|--backend)
        check_prerequisites
        start_backend
        wait_for_backend "$BACKEND_HOST" "$BACKEND_PORT"
        echo ""
        echo -e "${CLR_GREEN}✨ Запущен только Backend!${CLR_RESET}"
        echo "📡 REST API / WS: http://${BACKEND_HOST}:${BACKEND_PORT}"
        wait "$BACKEND_PID" 2>/dev/null || wait
        ;;
    all|*)
        check_prerequisites
        start_backend
        wait_for_backend "$BACKEND_HOST" "$BACKEND_PORT"
        start_frontend
        echo ""
        echo "========================================================"
        echo -e "${CLR_GREEN}  ✨ AetherCore Platform успешно запущен в dev-режиме!${CLR_RESET}"
        echo "  🌐 Веб-интерфейс: http://localhost:${FRONTEND_PORT} (HMR активен)"
        echo "  📡 REST API / WS: http://${BACKEND_HOST}:${BACKEND_PORT}"
        echo "  💡 Изменения в frontend/src применяются мгновенно"
        echo "  Для остановки нажмите Ctrl + C"
        echo "========================================================"
        echo ""
        wait -n "$BACKEND_PID" "$FRONTEND_PID" 2>/dev/null || wait
        ;;
esac

