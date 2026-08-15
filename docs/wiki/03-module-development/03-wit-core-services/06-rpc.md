# 🔄 WIT Интерфейс `nms:core/rpc`

## 1. Назначение и роль в ядре

Интерфейс `nms:core/rpc` обеспечивает безопасный синхронный вызов методов между изолированными плагинами через брокер микроядра (Inter-Module RPC).

---

## 2. Полный код WIT спецификации

```wit
interface rpc {
    /// Выполнить RPC-вызов другого модуля
    call: func(target-module: string, method: string, params-json: string) -> result<string, string>;
}
```

---

## 3. Пример использования на Rust (Гостевой код плагина)

```rust
use crate::bindings::nms::core::rpc;

pub fn fetch_device_info(device_ip: &str) -> Result<String, String> {
    let params = serde_json::json!({ "ip": device_ip }).to_string();
    rpc::call("network-topology", "get_device_details", &params)
}
```
