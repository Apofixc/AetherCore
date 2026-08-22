# 🔄 WIT Интерфейс `nms:core/rpc` — Межмодульные RPC Вызовы

## 1. Назначение и модель безопасности Inter-Module RPC

Интерфейс `nms:core/rpc` реализует синхронный механизм обмена данными между независимыми изолированными WASM-песочницами (Actor-to-Actor Inter-Process Communication).

### Принципы работы брокера RPC ядра:
1. **Прямое соединение запрещено**: Плагин $A$ не имеет доступа к адресному пространству памяти плагина $B$.
2. **Маршрутизация через ядро**: Вызов `rpc::call(target, method, params)` перехватывается хост-трамплином ядра, проверяется по графу зависимостей (`deps` / `optional_deps`), сериализуется и доставляется в очередь Tokio Mailbox целевого плагина $B$.
3. **Graceful Fallback при отключенном модуле**: Если целевой модуль отключен в системе или отсутствует, ядро возвращает ошибку `Err("Module 'X' is disabled or not found")`, не приводя к сбою вызывающего плагина.

---

## 2. Полный код WIT спецификации (`aethercore-core.wit`)

```wit
package nms:core@2.0.0;

interface rpc {
    /// Выполнить синхронный RPC-вызов метода другого плагина
    ///
    /// # Параметры:
    /// - `target-module`: идентификатор целевого модуля (например, "network-topology")
    /// - `method`: имя вызываемого метода
    /// - `params-json`: параметры вызова в формате валидной JSON-строки
    ///
    /// # Возвращаемое значение:
    /// - `ok(string)`: ответ целевого модуля в формате JSON
    /// - `err(string)`: ошибка (модуль не найден, метод не поддерживается, таймаут)
    call: func(target-module: string, method: string, params-json: string) -> result<string, string>;
}

interface rpc-handler {
    /// Гостевая точка входа: обработчик входящего RPC вызова от другого плагина
    ///
    /// # Параметры:
    /// - `caller`: идентификатор модуля, инициировавшего вызов
    /// - `method`: имя запрашиваемого метода
    /// - `params-json`: переданные параметры
    handle-rpc: func(caller: string, method: string, params-json: string) -> result<string, string>;
}
```

---

## 3. Практический пример: Вызов топологии из плагина мониторинга

### Клиент (Вызывающий плагин `dashboard-widget`):
```rust
use crate::bindings::nms::core::rpc;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct DeviceRequest {
    pub ip: String,
}

#[derive(Deserialize)]
struct DeviceDetails {
    pub hostname: String,
    pub vendor: String,
    pub uptime_sec: u64,
}

pub fn get_device_info(ip: &str) -> Result<DeviceDetails, String> {
    let req_payload = serde_json::to_string(&DeviceRequest { ip: ip.to_string() })
        .map_err(|e| e.to_string())?;

    // Вызов метода get_device в модуле network-topology
    let response_json = rpc::call("network-topology", "get_device", &req_payload)?;

    let details: DeviceDetails = serde_json::from_str(&response_json)
        .map_err(|e| format!("Invalid response format: {}", e))?;

    Ok(details)
}
```

### Сервер (Обработчик в плагине `network-topology`):
```rust
impl crate::bindings::exports::nms::core::rpc_handler::Guest for PluginComponent {
    fn handle_rpc(caller: String, method: String, params_json: String) -> Result<String, String> {
        match method.as_str() {
            "get_device" => {
                let req: DeviceRequest = serde_json::from_str(&params_json)
                    .map_err(|e| format!("Bad request: {}", e))?;

                let res = DeviceDetails {
                    hostname: format!("router-{}", req.ip.replace('.', "-")),
                    vendor: "Cisco".into(),
                    uptime_sec: 142050,
                };

                serde_json::to_string(&res).map_err(|e| e.to_string())
            }
            _ => Err(format!("Unknown RPC method: {}", method)),
        }
    }
}
```
