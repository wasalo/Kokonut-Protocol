use kokonut_protocol_near::{
    ChainCapabilities, ChainConfig, ChainReference, ExecutionRequestRecord, ExecutionStatus,
    SignatureStatus,
};
use near_kit::Near;
use near_sandbox::config::{DEFAULT_GENESIS_ACCOUNT, DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY};
use near_sandbox::{GenesisAccount, Sandbox, SandboxConfig};
use near_sdk::serde_json::json;
use std::fs;
use std::path::Path;

const SANDBOX_VERSION: &str = include_str!("../.near-sandbox-version");
const CONTRACT_ACCOUNT: &str = "sandbox";

#[tokio::test]
async fn deployed_hub_persists_execution_lifecycle_and_rejects_non_admin_mutation(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sandbox_binary = std::env::var("NEAR_SANDBOX_BIN_PATH")
        .expect("run through make test-near-sandbox to provide the checksum-pinned binary");
    assert!(
        Path::new(&sandbox_binary).is_file(),
        "pinned local sandbox binary must exist"
    );

    // A fresh local node and generated account keep state and keys test-scoped.
    let outsider_id = "outsider.sandbox"
        .parse()
        .expect("valid sandbox account ID");
    let outsider = GenesisAccount::generate_with_name(outsider_id);
    let operator_id = "operator.sandbox"
        .parse()
        .expect("valid sandbox account ID");
    let operator = GenesisAccount::generate_with_name(operator_id);
    let approver_id = "approver.sandbox"
        .parse()
        .expect("valid sandbox account ID");
    let approver = GenesisAccount::generate_with_name(approver_id);
    let mut config = SandboxConfig::default();
    config
        .additional_accounts
        .extend([outsider.clone(), operator.clone(), approver.clone()]);

    let sandbox =
        Sandbox::start_sandbox_with_config_and_version(config, SANDBOX_VERSION.trim()).await?;
    assert!(
        sandbox.rpc_addr.starts_with("http://127.0.0.1:"),
        "integration tests must use only the local sandbox RPC"
    );

    let administrator = Near::custom(&sandbox.rpc_addr, "sandbox")
        .credentials(
            DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY,
            DEFAULT_GENESIS_ACCOUNT.to_string(),
        )?
        .build();
    let non_administrator = Near::custom(&sandbox.rpc_addr, "sandbox")
        .credentials(&outsider.private_key, outsider.account_id.to_string())?
        .build();
    let execution_operator = Near::custom(&sandbox.rpc_addr, "sandbox")
        .credentials(&operator.private_key, operator.account_id.to_string())?
        .build();
    let execution_approver = Near::custom(&sandbox.rpc_addr, "sandbox")
        .credentials(&approver.private_key, approver.account_id.to_string())?
        .build();

    let wasm_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/deploy/kokonut_protocol_near.wasm");
    let wasm = fs::read(&wasm_path)?;

    let deployed = administrator.deploy(wasm).send().await?;
    assert!(
        deployed.is_success(),
        "Wasm deployment should succeed locally"
    );

    let initialized = administrator
        .call(CONTRACT_ACCOUNT, "new")
        .args(json!({ "administrator": CONTRACT_ACCOUNT }))
        .await?;
    assert!(
        initialized.is_success(),
        "contract initialization should succeed"
    );

    let registered = administrator
        .call(CONTRACT_ACCOUNT, "register_chain")
        .args(json!({
            "namespace": "eip155",
            "chain_id": "8453",
            "family": "evm",
            "signing_scheme": "secp256k1",
            "capabilities": {
                "chain_signatures": false,
                "contract_deploy": false,
                "eas": false,
                "safe": false,
                "baal": false,
                "intents_settlement": false
            },
            "enabled": true
        }))
        .await?;
    assert!(
        registered.is_success(),
        "administrator registration should persist"
    );
    let registration_block = registered
        .receipts_outcome
        .last()
        .expect("registration receipt outcome")
        .block_hash;

    let expected = ChainConfig {
        reference: ChainReference {
            namespace: "eip155".to_owned(),
            chain_id: "8453".to_owned(),
        },
        family: "evm".to_owned(),
        signing_scheme: "secp256k1".to_owned(),
        capabilities: ChainCapabilities {
            chain_signatures: false,
            contract_deploy: false,
            eas: false,
            safe: false,
            baal: false,
            intents_settlement: false,
        },
        enabled: true,
    };
    let stored: Option<ChainConfig> = administrator
        .view(CONTRACT_ACCOUNT, "get_chain")
        .args(json!({ "namespace": "eip155", "chain_id": "8453" }))
        .at_block_hash(registration_block)
        .await?;
    assert_eq!(stored, Some(expected.clone()));

    const PROJECT_ID: &str = "00000000-0000-4000-8000-000000000001";
    const REQUEST_ID: &str = "00000000-0000-4000-8000-000000000002";
    const TARGET: &str = "0x1234567890abcdef1234567890abcdef12345678";
    let project = administrator
        .call(CONTRACT_ACCOUNT, "register_project")
        .args(json!({
            "project_id": PROJECT_ID,
            "profile_hash": format!("0x{}", "d".repeat(64)),
            "approved_chains": [{ "namespace": "eip155", "chain_id": "8453" }]
        }))
        .await?;
    assert!(
        project.is_success(),
        "administrator should register a project"
    );

    let policy = administrator
        .call(CONTRACT_ACCOUNT, "configure_execution_policy")
        .args(json!({
            "project_id": PROJECT_ID,
            "namespace": "eip155",
            "chain_id": "8453",
            "version": "policy-v1",
            "allowed_operations": ["deploy_project_component"],
            "allowed_targets": [TARGET],
            "max_value_base_units": "1000",
            "operators": [operator.account_id.to_string()],
            "approvers": [approver.account_id.to_string()],
            "approval_threshold": 1
        }))
        .await?;
    assert!(
        policy.is_success(),
        "administrator should configure the policy"
    );

    let proposed = execution_operator
        .call(CONTRACT_ACCOUNT, "propose_execution")
        .args(json!({
            "request": {
                "protocol_version": "1",
                "request_id": REQUEST_ID,
                "project_id": PROJECT_ID,
                "chain_namespace": "eip155",
                "chain_id": "8453",
                "operation": "deploy_project_component",
                "target": TARGET,
                "value_base_units": "500",
                "gas_limit": "120000",
                "calldata_hash": format!("0x{}", "a".repeat(64)),
                "signing_payload_hash": format!("0x{}", "b".repeat(64)),
                "signer_key_version": 0,
                "expected_effect_hash": format!("0x{}", "c".repeat(64)),
                "nonce": "0",
                "gas_policy": { "max_fee": "10", "max_priority_fee": "2" },
                "expires_at": "2099-01-01T00:00:00Z",
                "policy_version": "policy-v1",
                "idempotency_key": "sandbox-execution-1"
            }
        }))
        .await?;
    assert!(
        proposed.is_success(),
        "authorized operator proposal should persist"
    );
    let proposal_block = proposed
        .receipts_outcome
        .last()
        .expect("proposal receipt outcome")
        .block_hash;
    let awaiting_approval: Option<ExecutionRequestRecord> = execution_operator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": REQUEST_ID }))
        .at_block_hash(proposal_block)
        .await?;
    let awaiting_approval = awaiting_approval.expect("proposed request should be stored");
    assert_eq!(awaiting_approval.status, ExecutionStatus::AwaitingApproval);
    assert_eq!(
        awaiting_approval.signature_status,
        SignatureStatus::NotRequested
    );
    assert_eq!(awaiting_approval.approval_count, 0);

    let approved = execution_approver
        .call(CONTRACT_ACCOUNT, "approve_execution")
        .args(json!({
            "request_id": REQUEST_ID,
            "expected_action_hash": awaiting_approval.action_hash
        }))
        .await?;
    assert!(
        approved.is_success(),
        "configured approver should approve the exact action"
    );
    let approval_block = approved
        .receipts_outcome
        .last()
        .expect("approval receipt outcome")
        .block_hash;
    let approved_record: Option<ExecutionRequestRecord> = administrator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": REQUEST_ID }))
        .at_block_hash(approval_block)
        .await?;
    let approved_record = approved_record.expect("approved request should be stored");
    assert_eq!(approved_record.status, ExecutionStatus::Approved);
    assert_eq!(approved_record.approval_count, 1);
    assert_eq!(
        approved_record.signature_status,
        SignatureStatus::NotRequested
    );

    let cancelled = execution_operator
        .call(CONTRACT_ACCOUNT, "cancel_execution")
        .args(json!({ "request_id": REQUEST_ID }))
        .await?;
    assert!(
        cancelled.is_success(),
        "the proposer should be able to cancel pre-signing"
    );
    let cancellation_block = cancelled
        .receipts_outcome
        .last()
        .expect("cancellation receipt outcome")
        .block_hash;
    let cancelled_record: Option<ExecutionRequestRecord> = administrator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": REQUEST_ID }))
        .at_block_hash(cancellation_block)
        .await?;
    let cancelled_record = cancelled_record.expect("cancelled request should remain auditable");
    assert_eq!(cancelled_record.status, ExecutionStatus::Cancelled);
    assert_eq!(
        cancelled_record.signature_status,
        SignatureStatus::NotRequested
    );

    let rejected = non_administrator
        .call(CONTRACT_ACCOUNT, "set_chain_enabled")
        .args(json!({
            "namespace": "eip155",
            "chain_id": "8453",
            "enabled": false
        }))
        .await?;
    assert!(
        rejected.is_failure(),
        "non-administrator mutation must fail"
    );
    let rejection_block = rejected
        .receipts_outcome
        .last()
        .expect("rejected mutation receipt outcome")
        .block_hash;

    let after_rejection: Option<ChainConfig> = administrator
        .view(CONTRACT_ACCOUNT, "get_chain")
        .args(json!({ "namespace": "eip155", "chain_id": "8453" }))
        .at_block_hash(rejection_block)
        .await?;
    assert_eq!(after_rejection, Some(expected));

    Ok(())
}
