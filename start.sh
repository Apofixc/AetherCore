#!/usr/bin/env bash
# ==============================================================================
# Скрипт запуска AetherCore NMS (Бэкенд Rust Axum + Фронтенд Vue 3 Vite Dev)
# ==============================================================================

set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

# Порты
BACKEND_PORT=3000
FRONTEND_PORT=5173
HOST="0.0.0.0"

MODE="${1:-all}" # all | frontend | backend

echo "========================================================"
echo "  🚀 Запуск AetherCore NMS (Development Mode)"
echo "========================================================"

# Освобождение занятых портов перед стартом (без pkill node)
free_port() {
    local port=$1
    if command -v fuser >/dev/null 2>&1; then
        fuser -k "${port}/tcp" 2>/dev/null || true
    fi
}

# Корректное завершение дочерних процессов при выходе
cleanup() {
    trap - INT TERM EXIT
    echo ""
    echo "🛑 Остановка сервисов AetherCore NMS..."
    kill 0 2>/dev/null || true
    wait 2>/dev/null || true
    echo "✓ Все процессы успешно остановлены."
    exit 0
}

trap cleanup INT TERM EXIT

# 1. Запуск Frontend
start_frontend() {
    if [ ! -d "frontend" ]; then
        echo "⚠️ Директория frontend не найдена. Запуск frontend пропущен."
        return 0
    fi

    echo "📦 Проверка зависимостей frontend..."
    if [ ! -d "frontend/node_modules" ]; then
        npm --prefix frontend install
    fi

    free_port $FRONTEND_PORT

    echo "🎨 Запуск Frontend Vite Dev сервера на http://localhost:${FRONTEND_PORT}..."
    npm --prefix frontend run dev -- --host ${HOST} --port ${FRONTEND_PORT} &
    FRONTEND_PID=$!
}

# 2. Запуск Backend
start_backend() {
    free_port $BACKEND_PORT

    echo "⚙️  Запуск Rust бэкенда на http://127.0.0.1:${BACKEND_PORT} (режим --dev)..."
    if command -v cargo-watch >/dev/null 2>&1; then
        cargo watch -q -c -w crates -x "run -p nms-cli -- --server --host 127.0.0.1 --port ${BACKEND_PORT} --dev" &
    else
        cargo run -p nms-cli -- --server --host 127.0.0.1 --port ${BACKEND_PORT} --dev &
    fi
    BACKEND_PID=$!
}

case "$MODE" in
    --frontend|frontend|-f)
        start_frontend
        echo ""
        echo "✨ Запущен только Frontend в режиме разработки (HMR включен)!"
        echo "🌐 Веб-интерфейс: http://localhost:${FRONTEND_PORT}"
        wait $FRONTEND_PID 2>/dev/null || wait
        ;;
    --backend|backend|-b)
        start_backend
        echo ""
        echo "✨ Запущен только Backend!"
        echo "📡 REST API / WS: http://127.0.0.1:${BACKEND_PORT}"
        wait $BACKEND_PID 2>/dev/null || wait
        ;;
    *)
        start_backend
        start_frontend
        echo ""
        echo "========================================================"
        echo "  ✨ AetherCore NMS успешно запущен в dev-режиме!"
        echo "  🌐 Веб-интерфейс: http://localhost:${FRONTEND_PORT} (HMR активен)"
        echo "  📡 REST API / WS: http://127.0.0.1:${BACKEND_PORT}"
        echo "  💡 Изменения в frontend/src применяются мгновенно"
        echo "  Для остановки нажмите Ctrl + C"
        echo "========================================================"
        echo ""
        wait -n $BACKEND_PID $FRONTEND_PID 2>/dev/null || wait
        ;;
esac
