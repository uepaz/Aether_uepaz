use super::{
    any, build_router_with_state, build_state_with_execution_runtime_override, hash_api_key, json,
    sample_local_openai_auth_snapshot, sample_local_openai_candidate_row,
    sample_local_openai_endpoint, sample_local_openai_key, sample_local_openai_provider,
    start_server, Arc, GatewayDataState, InMemoryAuthApiKeySnapshotRepository,
    InMemoryBillingReadRepository, InMemoryMinimalCandidateSelectionReadRepository,
    InMemoryProviderCatalogReadRepository, InMemoryRequestCandidateRepository,
    InMemoryUsageReadRepository, InMemoryWalletRepository, Json, Request, Router, StatusCode,
    StoredBillingModelContext, StoredWalletSnapshot, UsageReadRepository, UsageRuntimeConfig,
    WalletLookupKey, WalletReadRepository, DEVELOPMENT_ENCRYPTION_KEY, TRACE_ID_HEADER,
};
use crate::constants::{CONTROL_REQUEST_ID_HEADER, TUNNEL_AFFINITY_REQUEST_ID_HEADER};
use aether_data::repository::settlement::InMemorySettlementRepository;
use aether_data_contracts::repository::candidates::{
    RequestCandidateReadRepository, RequestCandidateStatus,
};
use std::sync::Mutex;

fn run_async_test_on_large_stack<F>(name: &'static str, future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let handle = std::thread::Builder::new()
        .name(name.to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime should build")
                .block_on(future);
        })
        .expect("large-stack test thread should spawn");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn gateway_settles_wallet_for_completed_execution_runtime_sync_usage() {
    run_async_test_on_large_stack(
        "gateway_settles_wallet_for_completed_execution_runtime_sync_usage",
        assert_wallet_usage_request_ids(1, false),
    );
}

#[test]
fn gateway_bills_repeated_client_trace_ids_as_separate_requests() {
    run_async_test_on_large_stack(
        "gateway_bills_repeated_client_trace_ids_as_separate_requests",
        assert_wallet_usage_request_ids(2, false),
    );
}

#[test]
fn gateway_keeps_billing_request_id_across_retries_and_failover() {
    run_async_test_on_large_stack(
        "gateway_keeps_billing_request_id_across_retries_and_failover",
        assert_wallet_usage_request_ids(1, true),
    );
}

async fn assert_wallet_usage_request_ids(request_count: usize, fail_primary: bool) {
    let usage_repository = Arc::new(InMemoryUsageReadRepository::default());
    let request_candidate_repository = Arc::new(InMemoryRequestCandidateRepository::default());
    let mut candidate_rows = vec![sample_local_openai_candidate_row()];
    let mut providers = vec![sample_local_openai_provider()];
    let mut endpoints = vec![sample_local_openai_endpoint()];
    let mut keys = vec![sample_local_openai_key()];
    if fail_primary {
        let mut candidate = sample_local_openai_candidate_row();
        candidate.provider_id = "provider-openai-usage-backup".to_string();
        candidate.endpoint_id = "endpoint-openai-usage-backup".to_string();
        candidate.key_id = "key-openai-usage-backup".to_string();
        candidate.provider_priority = 20;
        candidate.key_global_priority_by_format = Some(json!({"openai:chat": 2}));
        let mut provider = sample_local_openai_provider();
        provider.id = candidate.provider_id.clone();
        let mut endpoint = sample_local_openai_endpoint();
        endpoint.id = candidate.endpoint_id.clone();
        endpoint.provider_id = candidate.provider_id.clone();
        let mut key = sample_local_openai_key();
        key.id = candidate.key_id.clone();
        key.provider_id = candidate.provider_id.clone();
        key.global_priority_by_format = Some(json!({"openai:chat": 2}));
        providers.push(provider);
        endpoints.push(endpoint);
        keys.push(key);
        candidate_rows.push(candidate);
    }
    let billing_repository = Arc::new(InMemoryBillingReadRepository::seed(candidate_rows.iter().map(|candidate| {
        StoredBillingModelContext::new(
            candidate.provider_id.clone(),
            Some("pay_as_you_go".to_string()),
            Some(candidate.key_id.clone()),
            Some(json!({"openai:chat": 1.0})),
            Some(60),
            "global-model-openai-usage-local-1".to_string(),
            "gpt-5".to_string(),
            None,
            Some(0.02),
            Some(
                json!({"tiers":[{"up_to":null,"input_price_per_1m":3.0,"output_price_per_1m":15.0}]}),
            ),
            Some("model-openai-usage-local-1".to_string()),
            Some("gpt-5".to_string()),
            None,
            None,
            None,
        )
        .expect("billing context should build")
    })));
    let wallet_repository = Arc::new(InMemoryWalletRepository::seed(vec![
        StoredWalletSnapshot::new(
            "wallet-usage-sync-123".to_string(),
            Some("user-usage-sync-123".to_string()),
            None,
            10.0,
            2.0,
            "finite".to_string(),
            "USD".to_string(),
            "active".to_string(),
            0.0,
            0.0,
            0.0,
            0.0,
            100,
        )
        .expect("wallet should build"),
    ]));

    let upstream = Router::new().route(
        "/api/internal/gateway/report-sync",
        any(|_request: Request| async move { Json(json!({"ok": true})) }),
    );

    let attempt_request_ids = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed_attempt_ids = Arc::clone(&attempt_request_ids);
    let execution_runtime = Router::new().route(
        "/v1/execute/sync",
        any(move |request: Request| {
            let observed_attempt_ids = Arc::clone(&observed_attempt_ids);
            async move {
                let bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
                    .await
                    .expect("execution plan should read");
                let plan: serde_json::Value =
                    serde_json::from_slice(&bytes).expect("execution plan should parse");
                let request_id = plan["request_id"]
                    .as_str()
                    .expect("plan should have a request ID")
                    .to_string();
                let attempt = {
                    let mut ids = observed_attempt_ids
                        .lock()
                        .expect("attempt IDs should lock");
                    ids.push(request_id.clone());
                    ids.len()
                };
                // Two sticky attempts on the primary key fail before selecting the backup.
                if fail_primary && attempt <= 2 {
                    return Json(json!({
                        "request_id": request_id,
                        "status_code": 401,
                        "headers": { "content-type": "application/json" },
                        "body": { "json_body": { "error": { "message": "invalid auth token" } } },
                        "telemetry": { "elapsed_ms": 5 }
                    }));
                }
                Json(json!({
                    "request_id": request_id,
                    "status_code": 200,
                    "headers": {
                        "content-type": "application/json"
                    },
                    "body": {
                        "json_body": {
                            "id": "chatcmpl-usage-wallet-sync-123",
                            "usage": {
                                "prompt_tokens": 1000,
                                "completion_tokens": 500,
                                "total_tokens": 1500
                            }
                        }
                    },
                    "telemetry": {
                        "elapsed_ms": 45
                    }
                }))
            }
        }),
    );

    let (upstream_url, upstream_handle) = start_server(upstream).await;
    let (execution_runtime_url, execution_runtime_handle) = start_server(execution_runtime).await;
    let auth_repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::seed(vec![(
        Some(hash_api_key("sk-client-openai-wallet-sync")),
        sample_local_openai_auth_snapshot("api-key-usage-sync-123", "user-usage-sync-123"),
    )]));
    let candidate_selection_repository = Arc::new(
        InMemoryMinimalCandidateSelectionReadRepository::seed(candidate_rows),
    );
    let provider_catalog_repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        providers, endpoints, keys,
    ));
    let data_state = GatewayDataState::with_auth_candidate_selection_provider_catalog_request_candidates_usage_billing_and_wallet_for_tests(
        auth_repository,
        candidate_selection_repository,
        provider_catalog_repository,
        Arc::clone(&request_candidate_repository),
        usage_repository.clone(),
        billing_repository,
        wallet_repository.clone(),
        DEVELOPMENT_ENCRYPTION_KEY,
    )
    .with_system_config_values_for_tests(vec![("provider_priority_mode".to_string(), json!("global_key"))])
    .with_settlement_writer_for_tests(Arc::new(
        InMemorySettlementRepository::from_wallet_repository(wallet_repository.clone()),
    ));
    let gateway_state = build_state_with_execution_runtime_override(execution_runtime_url)
        .with_data_state_for_tests(data_state)
        .with_usage_runtime_for_tests(UsageRuntimeConfig {
            enabled: true,
            ..UsageRuntimeConfig::default()
        });
    let gateway = build_router_with_state(gateway_state);
    let (gateway_url, gateway_handle) = start_server(gateway).await;

    let client = reqwest::Client::new();
    let trace_id = "req-usage-wallet-sync-123";
    let forged_id = "a64ea35c-5919-4e05-8fd4-34845a6f35de";
    let mut request_ids = Vec::new();
    for _ in 0..request_count {
        let response = client
            .post(format!("{gateway_url}/v1/chat/completions"))
            .header(http::header::CONTENT_TYPE, "application/json")
            .header(
                http::header::AUTHORIZATION,
                "Bearer sk-client-openai-wallet-sync",
            )
            .header(TRACE_ID_HEADER, trace_id)
            // Clients cannot choose the internal billing ID by sending a response header either.
            .header(CONTROL_REQUEST_ID_HEADER, forged_id)
            .header(TUNNEL_AFFINITY_REQUEST_ID_HEADER, forged_id)
            .body("{\"model\":\"gpt-5\",\"messages\":[]}")
            .send()
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(TRACE_ID_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some(trace_id)
        );
        let request_id = response
            .headers()
            .get(CONTROL_REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .expect("response should expose its billing request ID")
            .to_string();
        let _: serde_json::Value = response.json().await.expect("response body should parse");
        request_ids.push(request_id);
    }
    assert_eq!(
        request_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        request_count,
        "distinct HTTP requests sharing a client trace must get distinct billing IDs"
    );
    for request_id in &request_ids {
        assert_ne!(request_id, trace_id);
        assert_ne!(request_id, forged_id);
        uuid::Uuid::parse_str(request_id).expect("billing ID should be a server-generated UUID");
    }

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let stored = loop {
        let stored = usage_repository
            .list_recent_usage_audits(Some("user-usage-sync-123"), request_count + 1)
            .await
            .expect("usage lookup should succeed");
        if stored.len() == request_count && stored.iter().all(|usage| usage.status == "completed") {
            break stored;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "each HTTP request should have its own completed usage record, observed: {stored:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    let expected_cost: f64 = stored.iter().map(|usage| usage.actual_total_cost_usd).sum();
    assert!(expected_cost > 0.0);
    for usage in &stored {
        assert!(request_ids.contains(&usage.request_id));
        assert_eq!(usage.total_tokens, 1500);
        assert!(usage.actual_total_cost_usd > 0.0);
        assert_eq!(
            usage
                .request_metadata
                .as_ref()
                .and_then(|value| value.get("trace_id"))
                .and_then(serde_json::Value::as_str),
            Some(trace_id)
        );
        let candidates = request_candidate_repository
            .list_by_request_id(&usage.request_id)
            .await
            .expect("candidate history should read");
        assert_eq!(candidates.len(), if fail_primary { 3 } else { 1 });
        assert_eq!(
            candidates.last().unwrap().status,
            RequestCandidateStatus::Success
        );
        if fail_primary {
            assert!(candidates[..2]
                .iter()
                .all(|candidate| candidate.status == RequestCandidateStatus::Failed));
        }
    }
    let observed = attempt_request_ids
        .lock()
        .expect("attempt IDs should lock")
        .clone();
    if fail_primary {
        assert_eq!(
            observed,
            vec![request_ids[0].clone(); 3],
            "same-key retries and candidate failover must keep the billing ID"
        );
    } else {
        assert_eq!(observed, request_ids);
    }

    let mut wallet = None;
    for _ in 0..1000 {
        wallet = wallet_repository
            .find(WalletLookupKey::UserId("user-usage-sync-123"))
            .await
            .expect("wallet lookup should succeed");
        if wallet
            .as_ref()
            .is_some_and(|wallet| (wallet.total_consumed - expected_cost).abs() < 1e-8)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let wallet = wallet.expect("wallet should exist");
    assert!((wallet.total_consumed - expected_cost).abs() < 1e-8,
        "wallet should settle every independent request exactly once, expected {expected_cost}, got {}", wallet.total_consumed);
    assert!((12.0 - wallet.balance - wallet.gift_balance - expected_cost).abs() < 1e-8);

    gateway_handle.abort();
    execution_runtime_handle.abort();
    upstream_handle.abort();
}
