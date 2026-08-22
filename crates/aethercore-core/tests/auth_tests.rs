//! # Тесты аутентификации, Argon2id и JWT токенов

use aethercore_core::auth::{hash_password, verify_password, JwtManager};
use uuid::Uuid;

#[test]
fn test_hash_and_verify_password() {
    let password = "SuperSecretPassword123!";
    let hash = hash_password(password).expect("Hashing failed");
    assert!(verify_password(password, &hash).unwrap());
    assert!(!verify_password("WrongPassword", &hash).unwrap());
}

#[test]
fn test_jwt_generation_and_verification() {
    let jwt_mgr = JwtManager::new("my-test-secret-key-12345", 3600);
    let user_id = Uuid::new_v4();

    let token = jwt_mgr
        .generate_token(
            user_id,
            "admin",
            true,
            vec!["users.manage".into(), "system.view".into()],
        )
        .expect("Token generation failed");

    let claims = jwt_mgr.verify_token(&token).expect("Token verification failed");
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.username, "admin");
    assert!(claims.is_superuser);
    assert_eq!(claims.permissions, vec!["users.manage", "system.view"]);
}
