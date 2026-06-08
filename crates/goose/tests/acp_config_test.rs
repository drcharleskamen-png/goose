#[allow(dead_code)]
#[path = "acp_common_tests/mod.rs"]
mod common_tests;

use common_tests::fixtures::server::AcpServerConnection;
use common_tests::fixtures::{run_test, send_custom, Connection, TestConnectionConfig};
use goose_test_support::EnforceSessionId;
use std::sync::Arc;

use common_tests::fixtures::OpenAiFixture;

#[test]
fn test_config_read_returns_current_state() {
    run_test(async move {
        let openai = OpenAiFixture::new(vec![], Arc::new(EnforceSessionId::default())).await;
        let conn = AcpServerConnection::new(TestConnectionConfig::default(), openai).await;

        let result = send_custom(
            conn.cx(),
            "_goose/unstable/config/read",
            serde_json::json!({}),
        )
        .await;
        assert!(result.is_ok(), "expected ok, got: {:?}", result);

        let response = result.unwrap();
        let obj = response
            .as_object()
            .expect("response should be a JSON object");
        assert!(
            !obj.is_empty(),
            "config/read should return a non-empty object"
        );
    });
}

#[test]
fn test_config_write_roundtrip() {
    run_test(async move {
        let openai = OpenAiFixture::new(vec![], Arc::new(EnforceSessionId::default())).await;
        let conn = AcpServerConnection::new(TestConnectionConfig::default(), openai).await;

        let write_result = send_custom(
            conn.cx(),
            "_goose/unstable/config/write",
            serde_json::json!({
                "GOOSE_CLI_THEME": "dark",
                "GOOSE_DEBUG": true,
            }),
        )
        .await;
        assert!(write_result.is_ok(), "write failed: {:?}", write_result);

        let response = write_result.unwrap();
        assert_eq!(
            response.get("GOOSE_CLI_THEME"),
            Some(&serde_json::json!("dark")),
            "write response should reflect the written value"
        );
        assert_eq!(
            response.get("GOOSE_DEBUG"),
            Some(&serde_json::json!(true)),
            "write response should reflect the written value"
        );

        let read_result = send_custom(
            conn.cx(),
            "_goose/unstable/config/read",
            serde_json::json!({}),
        )
        .await;
        assert!(read_result.is_ok(), "read failed: {:?}", read_result);

        let read_response = read_result.unwrap();
        assert_eq!(
            read_response.get("GOOSE_CLI_THEME"),
            Some(&serde_json::json!("dark")),
            "read after write should return the written value"
        );
        assert_eq!(
            read_response.get("GOOSE_DEBUG"),
            Some(&serde_json::json!(true)),
            "read after write should return the written value"
        );
    });
}

#[test]
fn test_config_write_sparse_patch() {
    run_test(async move {
        let openai = OpenAiFixture::new(vec![], Arc::new(EnforceSessionId::default())).await;
        let conn = AcpServerConnection::new(TestConnectionConfig::default(), openai).await;

        // Write two fields
        send_custom(
            conn.cx(),
            "_goose/unstable/config/write",
            serde_json::json!({
                "GOOSE_CLI_THEME": "light",
                "GOOSE_DEBUG": true,
            }),
        )
        .await
        .expect("initial write");

        // Write only one field — the other should remain unchanged
        let result = send_custom(
            conn.cx(),
            "_goose/unstable/config/write",
            serde_json::json!({
                "GOOSE_CLI_THEME": "dark",
            }),
        )
        .await
        .expect("sparse write");

        assert_eq!(
            result.get("GOOSE_CLI_THEME"),
            Some(&serde_json::json!("dark")),
            "sparse patch should update the specified field"
        );
        assert_eq!(
            result.get("GOOSE_DEBUG"),
            Some(&serde_json::json!(true)),
            "sparse patch should not clear unmentioned fields"
        );
    });
}

#[test]
fn test_config_write_extensions_roundtrip() {
    run_test(async move {
        let openai = OpenAiFixture::new(vec![], Arc::new(EnforceSessionId::default())).await;
        let conn = AcpServerConnection::new(TestConnectionConfig::default(), openai).await;

        let write_result = send_custom(
            conn.cx(),
            "_goose/unstable/config/write",
            serde_json::json!({
                "extensions": {
                    "my_builtin": {
                        "enabled": true,
                        "type": "builtin",
                        "name": "my_builtin",
                        "description": "A test builtin",
                        "display_name": null,
                        "timeout": null,
                        "bundled": null,
                        "available_tools": []
                    }
                }
            }),
        )
        .await;
        assert!(
            write_result.is_ok(),
            "extensions write failed: {:?}",
            write_result
        );

        let read_result = send_custom(
            conn.cx(),
            "_goose/unstable/config/read",
            serde_json::json!({}),
        )
        .await
        .expect("config/read after extensions write");

        let exts = read_result
            .get("extensions")
            .expect("extensions should be present in read response");
        assert!(
            exts.get("my_builtin").is_some(),
            "written extension should be readable, got: {:?}",
            exts
        );
    });
}

#[test]
fn test_config_write_providers_roundtrip() {
    run_test(async move {
        let openai = OpenAiFixture::new(vec![], Arc::new(EnforceSessionId::default())).await;
        let conn = AcpServerConnection::new(TestConnectionConfig::default(), openai).await;

        let write_result = send_custom(
            conn.cx(),
            "_goose/unstable/config/write",
            serde_json::json!({
                "providers": {
                    "anthropic": {
                        "enabled": true,
                        "model": "claude-opus-4-5",
                        "configured": true
                    }
                }
            }),
        )
        .await;
        assert!(
            write_result.is_ok(),
            "providers write failed: {:?}",
            write_result
        );

        let read_result = send_custom(
            conn.cx(),
            "_goose/unstable/config/read",
            serde_json::json!({}),
        )
        .await
        .expect("config/read after providers write");

        let providers = read_result
            .get("providers")
            .expect("providers should be present in read response");
        let anthropic = providers
            .get("anthropic")
            .expect("anthropic should be present");
        assert_eq!(
            anthropic.get("model"),
            Some(&serde_json::json!("claude-opus-4-5")),
            "provider model should survive write/read roundtrip"
        );
    });
}
