//! # Тесты подсистемы локализации i18n

use aethercore_common::i18n::{I18nRegistry, Locale};

#[test]
fn test_builtin_translations() {
    let registry = I18nRegistry::new();

    let ru = registry.translate(Locale::Ru, "core.ok", &[]);
    assert_eq!(ru, "Успешно");

    let en = registry.translate(Locale::En, "core.ok", &[]);
    assert_eq!(en, "Success");
}

#[test]
fn test_interpolation() {
    let registry = I18nRegistry::new();

    let err_ru = registry.translate(
        Locale::Ru,
        "core.error.not_found",
        &[("resource", "users/42")],
    );
    assert_eq!(err_ru, "Запрошенный ресурс 'users/42' не найден");

    let err_en = registry.translate(
        Locale::En,
        "core.error.not_found",
        &[("resource", "users/42")],
    );
    assert_eq!(err_en, "Requested resource 'users/42' was not found");
}

#[test]
fn test_register_plugin_json() {
    let registry = I18nRegistry::new();
    let plugin_ru_json = r#"{"widget.title": "Статус сети", "status.online": "В сети"}"#;

    let registered = registry
        .register_json(Locale::Ru, Some("my_plugin"), plugin_ru_json)
        .expect("Failed to register json");
    assert_eq!(registered, 2);

    let translated = registry.translate(Locale::Ru, "my_plugin.widget.title", &[]);
    assert_eq!(translated, "Статус сети");
}

#[test]
fn test_locale_parsing() {
    assert_eq!(Locale::from_str_relaxed("en-US,en;q=0.9"), Locale::En);
    assert_eq!(Locale::from_str_relaxed("ru-RU,ru;q=0.8"), Locale::Ru);
    assert_eq!(Locale::from_str_relaxed(""), Locale::Ru);
}
