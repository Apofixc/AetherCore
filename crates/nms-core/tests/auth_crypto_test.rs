// Unit-тесты для модулей аутентификации (auth.rs) и криптографии (crypto.rs)

use nms_core::{
    create_access_token, create_refresh_token, decode_token, decrypt_secret, encrypt_secret,
    generate_qr_svg, generate_totp_secret, get_totp_code, get_totp_uri, has_module_permission,
    has_permission, hash_password, is_ip_whitelisted, mask_secret, verify_password,
    verify_totp_code, WsTicketManager,
};

#[test]
fn test_argon2_hash_verify() {
    let password = "SecretPassword123!";
    let hash = hash_password(password).unwrap();

    assert!(verify_password(password, &hash));
    assert!(!verify_password("WrongPassword", &hash));
}

#[test]
fn test_jwt_token_flow() {
    let secret = "super-secret-key-1234567890";
    let token = create_access_token("usr-1", "admin", secret, 24).unwrap();

    let claims = decode_token(&token, secret).unwrap();
    assert_eq!(claims.sub, "usr-1");
    assert_eq!(claims.username, "admin");
    assert_eq!(claims.token_type, "access");

    // Проверка refresh токена
    let refresh_token = create_refresh_token("usr-1", "admin", &claims.jti, secret, 168).unwrap();
    let refresh_claims = decode_token(&refresh_token, secret).unwrap();
    assert_eq!(refresh_claims.token_type, "refresh");
}

#[test]
fn test_totp_2fa_flow() {
    let secret = generate_totp_secret();
    assert!(!secret.is_empty());

    let code = get_totp_code(&secret, None).unwrap();
    assert_eq!(code.len(), 6);

    assert!(verify_totp_code(&secret, &code));
    assert!(!verify_totp_code(&secret, "000000"));

    let uri = get_totp_uri("admin@nms.local", &secret, "NMS");
    assert!(uri.starts_with("otpauth://totp/"));
    assert!(uri.contains(&secret));
}

#[tokio::test]
async fn test_ws_ticket_manager() {
    let manager = WsTicketManager::new();
    let ticket = manager.create_ticket("usr-123").await;

    assert!(ticket.starts_with("wst_"));

    let user_id = manager.consume_ticket(&ticket).await;
    assert_eq!(user_id, Some("usr-123".to_string()));

    // Одноразовый билет нельзя использовать повторно
    let second_attempt = manager.consume_ticket(&ticket).await;
    assert_eq!(second_attempt, None);
}

#[test]
fn test_is_ip_whitelisted() {
    // Пустой список разрешает всё
    assert!(is_ip_whitelisted("192.168.1.50", ""));
    assert!(is_ip_whitelisted("10.0.0.1", "   "));

    // Проверка точных IP
    let wl1 = "192.168.1.100, 10.0.0.1; 127.0.0.1";
    assert!(is_ip_whitelisted("192.168.1.100", wl1));
    assert!(is_ip_whitelisted("10.0.0.1", wl1));
    assert!(!is_ip_whitelisted("192.168.1.101", wl1));

    // Проверка CIDR масок
    let wl_cidr = "192.168.1.0/24, 10.0.0.0/8, ::1";
    assert!(is_ip_whitelisted("192.168.1.42", wl_cidr));
    assert!(is_ip_whitelisted("10.255.0.1", wl_cidr));
    assert!(!is_ip_whitelisted("172.16.0.1", wl_cidr));
}

#[test]
fn test_rbac_permissions() {
    let perms_admin = vec!["system.all".to_string()];
    let perms_user = vec!["users.view".to_string(), "module.topology.view".to_string()];
    let perms_manager = vec!["users.manage".to_string()];

    // Проверка system.all
    assert!(has_permission(&perms_admin, "any.perm"));
    assert!(has_module_permission(&perms_admin, "topology", "edit"));

    // Проверка прямого разрешения
    assert!(has_permission(&perms_user, "users.view"));
    assert!(!has_permission(&perms_user, "users.manage"));

    // Проверка иерархии прав (manage включает view)
    assert!(has_permission(&perms_manager, "users.view"));
    assert!(has_permission(&perms_manager, "users.manage"));

    // Проверка модуль-специфичных прав
    assert!(has_module_permission(&perms_user, "topology", "view"));
    assert!(!has_module_permission(&perms_user, "topology", "edit"));
}

#[test]
fn test_generate_qr_svg() {
    let uri = "otpauth://totp/NMS:admin@nms.local?secret=JBSWY3DPEHPK3PXP";
    let svg_data_uri = generate_qr_svg(uri);

    assert!(svg_data_uri.starts_with("data:image/svg+xml;utf8,"));
    assert!(svg_data_uri.contains("svg"));
    assert!(svg_data_uri.contains("rect"));
}

#[test]
fn test_encrypt_decrypt_secret() {
    let secret_key = "app-master-key";
    let original = "db_password_super_secret";

    let encrypted = encrypt_secret(Some(original), secret_key).unwrap().unwrap();
    assert!(encrypted.starts_with("enc:v1:"));

    let decrypted = decrypt_secret(Some(&encrypted), secret_key)
        .unwrap()
        .unwrap();
    assert_eq!(decrypted, original);

    // Граничные случаи шифрования
    assert_eq!(encrypt_secret(None, secret_key).unwrap(), None);
    assert_eq!(
        encrypt_secret(Some(""), secret_key).unwrap(),
        Some("".to_string())
    );
    // Идемпотентность шифрования (уже зашифрованные данные не шифруются повторно)
    let double_enc = encrypt_secret(Some(&encrypted), secret_key)
        .unwrap()
        .unwrap();
    assert_eq!(double_enc, encrypted);

    // Граничные случаи расшифровки
    assert_eq!(decrypt_secret(None, secret_key).unwrap(), None);
    assert_eq!(
        decrypt_secret(Some(""), secret_key).unwrap(),
        Some("".to_string())
    );
    // Незашифрованная строка (фолбэк)
    assert_eq!(
        decrypt_secret(Some("plain_unencrypted"), secret_key).unwrap(),
        Some("plain_unencrypted".to_string())
    );
    // Невалидный base64 после enc:v1:
    assert_eq!(
        decrypt_secret(Some("enc:v1:???invalid_b64"), secret_key).unwrap(),
        Some("enc:v1:???invalid_b64".to_string())
    );
    // Слишком короткие декодированные данные (< 12 байт nonce)
    assert_eq!(
        decrypt_secret(Some("enc:v1:YWJj"), secret_key).unwrap(),
        Some("enc:v1:YWJj".to_string())
    );
    // Расшифровка с неверным ключом (фолбэк)
    assert_eq!(
        decrypt_secret(Some(&encrypted), "wrong-key").unwrap(),
        Some(encrypted)
    );
}

#[test]
fn test_mask_secret() {
    assert_eq!(mask_secret(Some("secret")), Some("***".to_string()));
    assert_eq!(mask_secret(Some("")), None);
    assert_eq!(mask_secret(None), None);
}

#[test]
fn test_decode_access_and_refresh_token() {
    use nms_core::{
        create_access_token, create_refresh_token, decode_access_token, decode_refresh_token,
        TOKEN_TTL_SECONDS,
    };
    assert_eq!(TOKEN_TTL_SECONDS, 604800);

    let secret = "secret-key-test-123456789";
    let acc_tok = create_access_token("usr-1", "alice", secret, 24).unwrap();
    let ref_tok = create_refresh_token("usr-1", "alice", "jti-100", secret, 168).unwrap();

    let acc_claims = decode_access_token(&acc_tok, secret).unwrap();
    assert_eq!(acc_claims.username, "alice");
    assert_eq!(acc_claims.token_type, "access");
    assert!(decode_refresh_token(&acc_tok, secret).is_none());

    let ref_claims = decode_refresh_token(&ref_tok, secret).unwrap();
    assert_eq!(ref_claims.jti, "jti-100");
    assert_eq!(ref_claims.token_type, "refresh");
    assert!(decode_access_token(&ref_tok, secret).is_none());
}

#[test]
fn test_is_origin_allowed() {
    use nms_core::is_origin_allowed;
    assert!(is_origin_allowed(None, None));
    assert!(is_origin_allowed(
        Some("http://localhost:3000"),
        Some(&["http://nms.local".to_string()])
    ));
    assert!(is_origin_allowed(
        Some("http://127.0.0.1:8080"),
        Some(&["http://nms.local".to_string()])
    ));
    assert!(is_origin_allowed(
        Some("http://192.168.1.10:3000"),
        Some(&["http://nms.local".to_string()])
    ));
    assert!(is_origin_allowed(
        Some("http://nms.local"),
        Some(&["http://nms.local".to_string()])
    ));
    assert!(!is_origin_allowed(
        Some("http://malicious.com"),
        Some(&["http://nms.local".to_string()])
    ));
}

#[tokio::test]
async fn test_standalone_ws_tickets() {
    use nms_core::{consume_ws_ticket, create_ws_ticket};
    let ticket = create_ws_ticket("usr-456", None, None).await;
    assert!(ticket.starts_with("wst_"));

    let consumed = consume_ws_ticket(&ticket).await;
    assert_eq!(consumed, Some("usr-456".to_string()));

    let repeat = consume_ws_ticket(&ticket).await;
    assert_eq!(repeat, None);
}

#[test]
fn test_require_permission_helpers() {
    use nms_core::{require_module_permission, require_permission, CurrentUser};
    let user = CurrentUser {
        id: "usr-1".to_string(),
        username: "admin".to_string(),
        full_name: "Admin User".to_string(),
        email: None,
        uid: None,
        role_id: "1".to_string(),
        role_name: "Admin".to_string(),
        avatar: None,
        is_authenticated: true,
        permissions: vec!["users.view".to_string(), "module.topology.view".to_string()],
        token_jti: None,
    };

    assert!(require_permission(&user, "users.view").is_ok());
    assert!(require_permission(&user, "users.manage").is_err());
    assert!(require_module_permission(&user, "topology", "view").is_ok());
    assert!(require_module_permission(&user, "topology", "edit").is_err());
}
