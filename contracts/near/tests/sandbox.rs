use kokonut_protocol_near::{
    ChainCapabilities, ChainConfig, ChainReference, ExecutionRequestRecord, ExecutionStatus,
    SignatureStatus,
};
use near_kit::{
    transaction::{Final, Included},
    CryptoHash, Near,
};
use near_sandbox::config::{DEFAULT_GENESIS_ACCOUNT, DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY};
use near_sandbox::{GenesisAccount, Sandbox, SandboxConfig};
use near_sdk::serde_json::json;
use std::fs;
use std::path::Path;

const SANDBOX_VERSION: &str = include_str!("../.near-sandbox-version");
const CONTRACT_ACCOUNT: &str = "sandbox";

async fn contract_storage_usage_at(
    near: &Near,
    block_hash: CryptoHash,
) -> Result<u64, near_kit::Error> {
    let account = near
        .account(CONTRACT_ACCOUNT)
        .at_block_hash(block_hash.clone())
        .await?;
    assert_eq!(
        account.block_hash, block_hash,
        "storage query must be block-pinned"
    );
    Ok(account.storage_usage)
}

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

#[tokio::test]
async fn deployed_hub_issues_and_persists_mock_chain_signature_callback(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use near_sdk::serde_json::{self, Value};
    use near_sdk::NearToken;

    const MOCK_SIGNER_ACCOUNT: &str = "mock-signer.testnet";
    const PROJECT_ID: &str = "00000000-0000-4000-8000-000000000011";
    const REQUEST_ID: &str = "00000000-0000-4000-8000-000000000012";
    const TARGET: &str = "0x1234567890abcdef1234567890abcdef12345678";
    const MOCK_PUBLIC_KEY: &str =
        "secp256k1:1111111111111111111111111111111111111111111111111111111111111111";

    let sandbox_binary = std::env::var("NEAR_SANDBOX_BIN_PATH")
        .expect("run through make test-near-sandbox to provide the checksum-pinned binary");
    assert!(Path::new(&sandbox_binary).is_file());

    let outsider = GenesisAccount::generate_with_name("outsider.sandbox".parse()?);
    let operator = GenesisAccount::generate_with_name("operator.sandbox".parse()?);
    let approver = GenesisAccount::generate_with_name("approver.sandbox".parse()?);
    let mock_signer = GenesisAccount::generate_with_name(MOCK_SIGNER_ACCOUNT.parse()?);
    let mut config = SandboxConfig::default();
    config.additional_accounts.extend([
        outsider.clone(),
        operator.clone(),
        approver.clone(),
        mock_signer.clone(),
    ]);

    let sandbox =
        Sandbox::start_sandbox_with_config_and_version(config, SANDBOX_VERSION.trim()).await?;
    assert!(sandbox.rpc_addr.starts_with("http://127.0.0.1:"));

    let administrator = Near::custom(&sandbox.rpc_addr, "sandbox")
        .credentials(
            DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY,
            DEFAULT_GENESIS_ACCOUNT.to_string(),
        )?
        .build();
    let execution_operator = Near::custom(&sandbox.rpc_addr, "sandbox")
        .credentials(&operator.private_key, operator.account_id.to_string())?
        .build();
    let execution_approver = Near::custom(&sandbox.rpc_addr, "sandbox")
        .credentials(&approver.private_key, approver.account_id.to_string())?
        .build();
    let mock_signer_client = Near::custom(&sandbox.rpc_addr, "sandbox")
        .credentials(&mock_signer.private_key, mock_signer.account_id.to_string())?
        .build();

    let hub_wasm_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/deploy/kokonut_protocol_near.wasm");
    let hub_deployment = administrator
        .deploy(fs::read(hub_wasm_path)?)
        .send()
        .await?;
    assert!(
        hub_deployment.is_success(),
        "Hub Wasm should deploy locally"
    );
    let hub_init = administrator
        .call(CONTRACT_ACCOUNT, "new")
        .args(json!({ "administrator": CONTRACT_ACCOUNT }))
        .await?;
    assert!(hub_init.is_success(), "Hub should initialize locally");

    let mock_wasm_path = std::env::var("MOCK_CHAIN_SIGNATURE_WASM_PATH")
        .expect("Make must provide the locally built mock signer Wasm path");
    assert!(Path::new(&mock_wasm_path).is_file());
    let mock_deployment = mock_signer_client
        .deploy(fs::read(mock_wasm_path)?)
        .send()
        .await?;
    assert!(
        mock_deployment.is_success(),
        "mock signer should deploy locally"
    );
    let mock_init = mock_signer_client
        .call(MOCK_SIGNER_ACCOUNT, "new")
        .args(json!({}))
        .await?;
    assert!(
        mock_init.is_success(),
        "mock signer should initialize locally"
    );

    let registered_chain = administrator
        .call(CONTRACT_ACCOUNT, "register_chain")
        .args(json!({
            "namespace": "eip155",
            "chain_id": "8453",
            "family": "evm",
            "signing_scheme": "secp256k1",
            "capabilities": {
                "chain_signatures": true,
                "contract_deploy": false,
                "eas": false,
                "safe": false,
                "baal": false,
                "intents_settlement": false
            },
            "enabled": true
        }))
        .await?;
    assert!(registered_chain.is_success());

    let signer_config = administrator
        .call(CONTRACT_ACCOUNT, "configure_chain_signatures")
        .args(json!({ "contract_id": MOCK_SIGNER_ACCOUNT, "public_key": MOCK_PUBLIC_KEY }))
        .await?;
    assert!(signer_config.is_success());
    let project = administrator
        .call(CONTRACT_ACCOUNT, "register_project")
        .args(json!({
            "project_id": PROJECT_ID,
            "profile_hash": format!("0x{}", "d".repeat(64)),
            "approved_chains": [{ "namespace": "eip155", "chain_id": "8453" }]
        }))
        .await?;
    assert!(project.is_success());
    let derivation_path = format!("kokonut/v1/project/{PROJECT_ID}/eip155/8453/deployer");
    let signer_binding = administrator
        .call(CONTRACT_ACCOUNT, "configure_signer")
        .args(json!({
            "project_id": PROJECT_ID,
            "namespace": "eip155",
            "chain_id": "8453",
            "purpose": "deployer",
            "derivation_path": derivation_path,
            "signing_scheme": "secp256k1",
            "derived_address": TARGET,
            "key_version": 0
        }))
        .await?;
    assert!(signer_binding.is_success());
    let activate_signer = administrator
        .call(CONTRACT_ACCOUNT, "activate_signer")
        .args(json!({
            "project_id": PROJECT_ID,
            "namespace": "eip155",
            "chain_id": "8453",
            "purpose": "deployer",
            "key_version": 0
        }))
        .await?;
    assert!(activate_signer.is_success());
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
    assert!(policy.is_success());
    let configured_block = policy
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub policy-configuration receipt")
        .block_hash;
    let mut storage_points = vec![(
        "configured_baseline",
        contract_storage_usage_at(&administrator, configured_block).await?,
    )];

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
                "idempotency_key": "sandbox-mock-signer-1"
            }
        }))
        .await?;
    assert!(proposed.is_success());
    let proposal_block = proposed
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub proposal receipt")
        .block_hash;
    storage_points.push((
        "request_awaiting_approval",
        contract_storage_usage_at(&administrator, proposal_block.clone()).await?,
    ));
    let proposed_record: Option<ExecutionRequestRecord> = execution_operator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": REQUEST_ID }))
        .at_block_hash(proposal_block)
        .await?;
    let proposed_record = proposed_record.expect("proposal should persist");
    assert_eq!(proposed_record.status, ExecutionStatus::AwaitingApproval);
    assert_eq!(
        proposed_record.signature_status,
        SignatureStatus::NotRequested
    );

    let approval = execution_approver
        .call(CONTRACT_ACCOUNT, "approve_execution")
        .args(json!({
            "request_id": REQUEST_ID,
            "expected_action_hash": proposed_record.action_hash
        }))
        .await?;
    assert!(approval.is_success());
    let approval_block = approval
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub approval receipt")
        .block_hash;
    storage_points.push((
        "request_approved",
        contract_storage_usage_at(&administrator, approval_block).await?,
    ));

    let signature_request = execution_operator
        .call(CONTRACT_ACCOUNT, "request_signature")
        .args(json!({
            "request_id": REQUEST_ID,
            "expected_action_hash": proposed_record.action_hash
        }))
        .gas("300 Tgas")
        .deposit(NearToken::from_near(1))
        .send()
        .wait_until::<Included>()
        .await?;
    let mut pending_signature_block = None;
    let mut recorded_request: Option<Value> = None;
    for _ in 0..20 {
        let progress = execution_operator
            .tx_status(
                &signature_request.transaction_hash,
                &signature_request.sender_id,
            )
            .wait_until::<Included>()
            .await?;
        if let Some(outcome) = progress.outcome.as_ref() {
            let signer_block = outcome
                .receipts_outcome
                .iter()
                .rev()
                .find(|receipt| receipt.outcome.executor_id.as_str() == MOCK_SIGNER_ACCOUNT)
                .map(|receipt| receipt.block_hash.clone());
            if let Some(signer_block) = signer_block {
                recorded_request = mock_signer_client
                    .view(MOCK_SIGNER_ACCOUNT, "get_last_request")
                    .args(json!({}))
                    .at_block_hash(signer_block)
                    .await?;
                if recorded_request.is_some() {
                    if let Some(hub_block) = outcome
                        .receipts_outcome
                        .iter()
                        .rev()
                        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
                        .map(|receipt| receipt.block_hash.clone())
                    {
                        pending_signature_block = Some(hub_block);
                        break;
                    }
                }
            }
        }
        sandbox.fast_forward(1).await?;
    }
    let pending_signature_block =
        pending_signature_block.expect("mock signer receipt should be included");
    let recorded_request =
        recorded_request.expect("mock signer should receive the pending request");
    assert_eq!(recorded_request["path"], derivation_path);
    assert_eq!(recorded_request["key_version"], 0);
    assert_eq!(recorded_request["payload"], json!(vec![0xbbu8; 32]));

    let pending_record: Option<ExecutionRequestRecord> = administrator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": REQUEST_ID }))
        .at_block_hash(pending_signature_block.clone())
        .await?;
    let pending_record = pending_record.expect("pending request should persist");
    assert_eq!(pending_record.status, ExecutionStatus::Approved);
    assert_eq!(pending_record.signature_status, SignatureStatus::Pending);
    storage_points.push((
        "request_signature_pending",
        contract_storage_usage_at(&administrator, pending_signature_block.clone()).await?,
    ));

    let resumed = mock_signer_client
        .call(MOCK_SIGNER_ACCOUNT, "resume_signature")
        .args(json!({
            "signature": {
                "big_r": { "affine_point": format!("04{}", "11".repeat(64)) },
                "s": { "scalar": "22".repeat(32) },
                "recovery_id": 0
            }
        }))
        .await?;
    assert!(resumed.is_success(), "test should resolve the mock yield");
    let completed_signature_tx = execution_operator
        .tx_status(
            &signature_request.transaction_hash,
            &signature_request.sender_id,
        )
        .wait_until::<Final>()
        .await?;
    assert!(
        completed_signature_tx.is_success(),
        "signature callback transaction should complete"
    );
    let completed_signature_block = completed_signature_tx
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub signature callback receipt")
        .block_hash;
    let issued_record: Option<ExecutionRequestRecord> = administrator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": REQUEST_ID }))
        .at_block_hash(completed_signature_block.clone())
        .await?;
    let issued_record = issued_record.expect("signed execution request should persist");
    assert_eq!(issued_record.status, ExecutionStatus::Approved);
    assert_eq!(issued_record.signature_status, SignatureStatus::Issued);
    assert_eq!(
        issued_record.signature_public_key.as_deref(),
        Some(MOCK_PUBLIC_KEY)
    );
    let signature: Value = serde_json::from_str(
        issued_record
            .signature_response
            .as_deref()
            .expect("mock callback should persist its response"),
    )?;
    assert_eq!(
        signature["big_r"]["affine_point"].as_str().unwrap().len(),
        130
    );
    assert_eq!(signature["s"]["scalar"].as_str().unwrap().len(), 64);
    assert_eq!(signature["recovery_id"], 0);
    storage_points.push((
        "request_signature_issued",
        contract_storage_usage_at(&administrator, completed_signature_block).await?,
    ));

    const CANCELLED_REQUEST_ID: &str = "00000000-0000-4000-8000-000000000013";
    let proposed_for_cancellation = execution_operator
        .call(CONTRACT_ACCOUNT, "propose_execution")
        .args(json!({
            "request": {
                "protocol_version": "1",
                "request_id": CANCELLED_REQUEST_ID,
                "project_id": PROJECT_ID,
                "chain_namespace": "eip155",
                "chain_id": "8453",
                "operation": "deploy_project_component",
                "target": TARGET,
                "value_base_units": "500",
                "gas_limit": "120000",
                "calldata_hash": format!("0x{}", "a".repeat(64)),
                "signing_payload_hash": format!("0x{}", "e".repeat(64)),
                "signer_key_version": 0,
                "expected_effect_hash": format!("0x{}", "c".repeat(64)),
                "nonce": "0",
                "gas_policy": { "max_fee": "10", "max_priority_fee": "2" },
                "expires_at": "2099-01-01T00:00:00Z",
                "policy_version": "policy-v1",
                "idempotency_key": "sandbox-mock-signer-cancel"
            }
        }))
        .await?;
    assert!(proposed_for_cancellation.is_success());
    let cancellation_proposal_block = proposed_for_cancellation
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub cancellation proposal receipt")
        .block_hash;
    storage_points.push((
        "second_request_awaiting_approval",
        contract_storage_usage_at(&administrator, cancellation_proposal_block.clone()).await?,
    ));
    let cancel_proposal: Option<ExecutionRequestRecord> = execution_operator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": CANCELLED_REQUEST_ID }))
        .at_block_hash(cancellation_proposal_block)
        .await?;
    let cancel_proposal = cancel_proposal.expect("cancellable proposal should persist");
    let cancel_approval = execution_approver
        .call(CONTRACT_ACCOUNT, "approve_execution")
        .args(json!({
            "request_id": CANCELLED_REQUEST_ID,
            "expected_action_hash": cancel_proposal.action_hash
        }))
        .await?;
    assert!(cancel_approval.is_success());
    let cancellation_approval_block = cancel_approval
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub cancellation-path approval receipt")
        .block_hash;
    storage_points.push((
        "second_request_approved",
        contract_storage_usage_at(&administrator, cancellation_approval_block).await?,
    ));
    let cancellation_signature_request = execution_operator
        .call(CONTRACT_ACCOUNT, "request_signature")
        .args(json!({
            "request_id": CANCELLED_REQUEST_ID,
            "expected_action_hash": cancel_proposal.action_hash
        }))
        .gas("300 Tgas")
        .deposit(NearToken::from_near(1))
        .send()
        .wait_until::<Included>()
        .await?;

    let mut cancellation_hub_block = None;
    let mut cancellation_mock_request: Option<Value> = None;
    for _ in 0..20 {
        let progress = execution_operator
            .tx_status(
                &cancellation_signature_request.transaction_hash,
                &cancellation_signature_request.sender_id,
            )
            .wait_until::<Included>()
            .await?;
        if let Some(outcome) = progress.outcome.as_ref() {
            if let Some(signer_block) = outcome
                .receipts_outcome
                .iter()
                .rev()
                .find(|receipt| receipt.outcome.executor_id.as_str() == MOCK_SIGNER_ACCOUNT)
                .map(|receipt| receipt.block_hash.clone())
            {
                cancellation_mock_request = mock_signer_client
                    .view(MOCK_SIGNER_ACCOUNT, "get_last_request")
                    .args(json!({}))
                    .at_block_hash(signer_block)
                    .await?;
                if cancellation_mock_request
                    .as_ref()
                    .is_some_and(|request| request["payload"] == json!(vec![0xeeu8; 32]))
                {
                    cancellation_hub_block = outcome
                        .receipts_outcome
                        .iter()
                        .rev()
                        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
                        .map(|receipt| receipt.block_hash.clone());
                    if cancellation_hub_block.is_some() {
                        break;
                    }
                }
            }
        }
        sandbox.fast_forward(1).await?;
    }
    let cancellation_hub_block =
        cancellation_hub_block.expect("Hub request-signature receipt should be included");
    let cancellation_mock_request = cancellation_mock_request
        .expect("mock signer should receive the cancellation-path request");
    assert_eq!(
        cancellation_mock_request["payload"],
        json!(vec![0xeeu8; 32])
    );
    let pending_cancellation_record: Option<ExecutionRequestRecord> = administrator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": CANCELLED_REQUEST_ID }))
        .at_block_hash(cancellation_hub_block.clone())
        .await?;
    let pending_cancellation_record =
        pending_cancellation_record.expect("signature-pending request should persist");
    assert_eq!(
        pending_cancellation_record.signature_status,
        SignatureStatus::Pending
    );
    storage_points.push((
        "second_request_signature_pending",
        contract_storage_usage_at(&administrator, cancellation_hub_block).await?,
    ));

    let cancelled = execution_operator
        .call(CONTRACT_ACCOUNT, "cancel_execution")
        .args(json!({ "request_id": CANCELLED_REQUEST_ID }))
        .await?;
    assert!(cancelled.is_success());
    let cancelled_block = cancelled
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub cancellation receipt")
        .block_hash;
    let cancelled_storage =
        contract_storage_usage_at(&administrator, cancelled_block.clone()).await?;
    storage_points.push(("second_request_cancelled", cancelled_storage));
    let cancelled_record: Option<ExecutionRequestRecord> = administrator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": CANCELLED_REQUEST_ID }))
        .at_block_hash(cancelled_block)
        .await?;
    let cancelled_record = cancelled_record.expect("cancelled request should persist");
    assert_eq!(cancelled_record.status, ExecutionStatus::Cancelled);
    assert_eq!(cancelled_record.signature_status, SignatureStatus::Failed);
    assert!(cancelled_record.signature_response.is_none());

    let late_resume = mock_signer_client
        .call(MOCK_SIGNER_ACCOUNT, "resume_signature")
        .args(json!({
            "signature": {
                "big_r": { "affine_point": format!("04{}", "11".repeat(64)) },
                "s": { "scalar": "22".repeat(32) },
                "recovery_id": 0
            }
        }))
        .await?;
    assert!(
        late_resume.is_success(),
        "test should resolve the late mock callback"
    );
    let completed_cancelled_signature_tx = execution_operator
        .tx_status(
            &cancellation_signature_request.transaction_hash,
            &cancellation_signature_request.sender_id,
        )
        .wait_until::<Final>()
        .await?;
    assert!(
        completed_cancelled_signature_tx.is_success(),
        "late callback transaction chain should complete successfully"
    );
    let completed_cancelled_callback_block = completed_cancelled_signature_tx
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub late signature callback receipt")
        .block_hash;
    let after_late_callback: Option<ExecutionRequestRecord> = administrator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": CANCELLED_REQUEST_ID }))
        .at_block_hash(completed_cancelled_callback_block.clone())
        .await?;
    let after_late_callback = after_late_callback.expect("cancelled record should remain");
    assert_eq!(after_late_callback.status, ExecutionStatus::Cancelled);
    assert_eq!(
        after_late_callback.signature_status,
        SignatureStatus::Failed
    );
    assert!(after_late_callback.signature_response.is_none());
    let late_callback_storage =
        contract_storage_usage_at(&administrator, completed_cancelled_callback_block).await?;
    storage_points.push(("late_callback_after_cancel", late_callback_storage));
    assert_eq!(
        late_callback_storage, cancelled_storage,
        "a late callback after cancellation must not grow Hub state"
    );

    const BOUNDARY_PROJECT_ID: &str = "00000000-0000-4000-8000-000000000021";
    const BOUNDARY_REQUEST_IDS: [&str; 3] = [
        "00000000-0000-4000-8000-000000000022",
        "00000000-0000-4000-8000-000000000023",
        "00000000-0000-4000-8000-000000000024",
    ];
    let max_namespace = "a".repeat(64);
    let max_chain_id = "1".repeat(128);
    let max_operation = format!("a{}", "b".repeat(79));
    let max_target = "T".repeat(256);
    let max_policy_version = format!("p{}", "v".repeat(127));
    let max_uint256 =
        "115792089237316195423570985008687907853269984665640564039457584007913129639935";

    let boundary_chain = administrator
        .call(CONTRACT_ACCOUNT, "register_chain")
        .args(json!({
            "namespace": max_namespace.clone(),
            "chain_id": max_chain_id.clone(),
            "family": "other",
            "signing_scheme": "other",
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
        boundary_chain.is_success(),
        "maximum chain identifiers register"
    );
    let boundary_project = administrator
        .call(CONTRACT_ACCOUNT, "register_project")
        .args(json!({
            "project_id": BOUNDARY_PROJECT_ID,
            "profile_hash": format!("0x{}", "f".repeat(64)),
            "approved_chains": [{
                "namespace": max_namespace.clone(),
                "chain_id": max_chain_id.clone()
            }]
        }))
        .await?;
    assert!(boundary_project.is_success());
    let boundary_policy = administrator
        .call(CONTRACT_ACCOUNT, "configure_execution_policy")
        .args(json!({
            "project_id": BOUNDARY_PROJECT_ID,
            "namespace": max_namespace.clone(),
            "chain_id": max_chain_id.clone(),
            "version": max_policy_version.clone(),
            "allowed_operations": [max_operation.clone()],
            "allowed_targets": [max_target.clone()],
            "max_value_base_units": max_uint256,
            "operators": [operator.account_id.to_string()],
            "approvers": [approver.account_id.to_string()],
            "approval_threshold": 1
        }))
        .await?;
    assert!(boundary_policy.is_success());
    let boundary_policy_block = boundary_policy
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub maximum-input policy receipt")
        .block_hash;
    storage_points.push((
        "max_input_configured_baseline",
        contract_storage_usage_at(&administrator, boundary_policy_block).await?,
    ));

    for (index, (request_id, idempotency_prefix)) in
        BOUNDARY_REQUEST_IDS.iter().zip(["a", "b", "c"]).enumerate()
    {
        let idempotency_key = format!("{idempotency_prefix}{}", "i".repeat(255));
        let proposed = execution_operator
            .call(CONTRACT_ACCOUNT, "propose_execution")
            .args(json!({
                "request": {
                    "protocol_version": "1",
                    "request_id": request_id,
                    "project_id": BOUNDARY_PROJECT_ID,
                    "chain_namespace": max_namespace,
                    "chain_id": max_chain_id,
                    "operation": max_operation,
                    "target": max_target,
                    "value_base_units": max_uint256,
                    "calldata_hash": format!("0x{}", "a".repeat(64)),
                    "nonce": max_uint256,
                    "gas_policy": {
                        "max_fee": max_uint256,
                        "max_priority_fee": max_uint256
                    },
                    "expires_at": "2099-01-01T00:00:00.123456789+00:00",
                    "policy_version": max_policy_version,
                    "idempotency_key": idempotency_key
                }
            }))
            .await?;
        assert!(
            proposed.is_success(),
            "maximum-bounded request {} should persist",
            index + 1
        );
        let request_block = proposed
            .receipts_outcome
            .iter()
            .rev()
            .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
            .expect("Hub maximum-request receipt")
            .block_hash;
        let stored: Option<ExecutionRequestRecord> = execution_operator
            .view(CONTRACT_ACCOUNT, "get_execution_request")
            .args(json!({ "request_id": request_id }))
            .at_block_hash(request_block.clone())
            .await?;
        let stored = stored.expect("maximum-bounded request remains retained");
        assert_eq!(stored.request.chain_namespace.len(), 64);
        assert_eq!(stored.request.chain_id.len(), 128);
        assert_eq!(stored.request.operation.len(), 80);
        assert_eq!(stored.request.target.len(), 256);
        assert_eq!(stored.request.value_base_units, max_uint256);
        assert_eq!(stored.request.nonce, max_uint256);
        assert_eq!(stored.request.gas_policy.max_fee, max_uint256);
        assert_eq!(stored.request.policy_version.len(), 128);
        assert_eq!(stored.request.idempotency_key.len(), 256);
        let phase = match index {
            0 => "max_request_1_retained",
            1 => "max_request_2_retained",
            2 => "max_request_3_retained",
            _ => unreachable!("fixed boundary request count"),
        };
        storage_points.push((
            phase,
            contract_storage_usage_at(&administrator, request_block).await?,
        ));
    }

    const MAX_SIGNATURE_REQUEST_ID: &str = "00000000-0000-4000-8000-000000000025";
    let maximum_signature_proposal = execution_operator
        .call(CONTRACT_ACCOUNT, "propose_execution")
        .args(json!({
            "request": {
                "protocol_version": "1",
                "request_id": MAX_SIGNATURE_REQUEST_ID,
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
                "nonce": "1",
                "gas_policy": { "max_fee": "10", "max_priority_fee": "2" },
                "expires_at": "2099-01-01T00:00:00Z",
                "policy_version": "policy-v1",
                "idempotency_key": format!("z{}", "x".repeat(255))
            }
        }))
        .await?;
    assert!(maximum_signature_proposal.is_success());
    let maximum_signature_proposal_block = maximum_signature_proposal
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub maximum-signature proposal receipt")
        .block_hash;
    let maximum_signature_record: Option<ExecutionRequestRecord> = execution_operator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": MAX_SIGNATURE_REQUEST_ID }))
        .at_block_hash(maximum_signature_proposal_block.clone())
        .await?;
    let maximum_signature_record =
        maximum_signature_record.expect("maximum-key signature request persists");
    assert_eq!(maximum_signature_record.request.idempotency_key.len(), 256);
    storage_points.push((
        "max_signature_request_awaiting_approval",
        contract_storage_usage_at(&administrator, maximum_signature_proposal_block).await?,
    ));

    let maximum_signature_approval = execution_approver
        .call(CONTRACT_ACCOUNT, "approve_execution")
        .args(json!({
            "request_id": MAX_SIGNATURE_REQUEST_ID,
            "expected_action_hash": maximum_signature_record.action_hash
        }))
        .await?;
    assert!(maximum_signature_approval.is_success());
    let maximum_signature_approval_block = maximum_signature_approval
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub maximum-signature approval receipt")
        .block_hash;
    storage_points.push((
        "max_signature_request_approved",
        contract_storage_usage_at(&administrator, maximum_signature_approval_block).await?,
    ));

    let maximum_signature_request = execution_operator
        .call(CONTRACT_ACCOUNT, "request_signature")
        .args(json!({
            "request_id": MAX_SIGNATURE_REQUEST_ID,
            "expected_action_hash": maximum_signature_record.action_hash
        }))
        .gas("300 Tgas")
        .deposit(NearToken::from_near(1))
        .send()
        .wait_until::<Included>()
        .await?;
    let mut maximum_signature_pending_block = None;
    for _ in 0..20 {
        let progress = execution_operator
            .tx_status(
                &maximum_signature_request.transaction_hash,
                &maximum_signature_request.sender_id,
            )
            .wait_until::<Included>()
            .await?;
        if let Some(outcome) = progress.outcome.as_ref() {
            let signer_block = outcome
                .receipts_outcome
                .iter()
                .rev()
                .find(|receipt| receipt.outcome.executor_id.as_str() == MOCK_SIGNER_ACCOUNT)
                .map(|receipt| receipt.block_hash.clone());
            if let Some(signer_block) = signer_block {
                let mock_request: Option<Value> = mock_signer_client
                    .view(MOCK_SIGNER_ACCOUNT, "get_last_request")
                    .args(json!({}))
                    .at_block_hash(signer_block)
                    .await?;
                if mock_request
                    .as_ref()
                    .is_some_and(|request| request["payload"] == json!(vec![0xbbu8; 32]))
                {
                    maximum_signature_pending_block = outcome
                        .receipts_outcome
                        .iter()
                        .rev()
                        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
                        .map(|receipt| receipt.block_hash.clone());
                    if maximum_signature_pending_block.is_some() {
                        break;
                    }
                }
            }
        }
        sandbox.fast_forward(1).await?;
    }
    let maximum_signature_pending_block = maximum_signature_pending_block
        .expect("Hub maximum-signature pending receipt should be included");
    storage_points.push((
        "max_signature_request_pending",
        contract_storage_usage_at(&administrator, maximum_signature_pending_block).await?,
    ));

    let mut maximum_signature: Value = json!({
        "big_r": { "affine_point": "" },
        "s": { "scalar": "22" },
        "recovery_id": 10
    });
    let signature_envelope_len = serde_json::to_vec(&maximum_signature)?.len();
    // NEAR's local yield-resume host rejects payloads above 1,024 bytes.
    let affine_point_len = 1024usize
        .checked_sub(signature_envelope_len)
        .expect("signature JSON envelope is smaller than sandbox yield limit");
    assert_eq!(affine_point_len % 2, 0);
    maximum_signature["big_r"]["affine_point"] = Value::String("11".repeat(affine_point_len / 2));
    assert_eq!(serde_json::to_vec(&maximum_signature)?.len(), 1024);
    let maximum_signature_resume = mock_signer_client
        .call(MOCK_SIGNER_ACCOUNT, "resume_signature")
        .args(json!({ "signature": maximum_signature }))
        .await?;
    assert!(
        maximum_signature_resume.is_success(),
        "1,024-byte mock callback resume failed: {maximum_signature_resume:?}"
    );
    let maximum_signature_completed = execution_operator
        .tx_status(
            &maximum_signature_request.transaction_hash,
            &maximum_signature_request.sender_id,
        )
        .wait_until::<Final>()
        .await?;
    assert!(maximum_signature_completed.is_success());
    let maximum_signature_callback_block = maximum_signature_completed
        .receipts_outcome
        .iter()
        .rev()
        .find(|receipt| receipt.outcome.executor_id.as_str() == CONTRACT_ACCOUNT)
        .expect("Hub maximum-signature callback receipt")
        .block_hash;
    let maximum_signature_issued: Option<ExecutionRequestRecord> = administrator
        .view(CONTRACT_ACCOUNT, "get_execution_request")
        .args(json!({ "request_id": MAX_SIGNATURE_REQUEST_ID }))
        .at_block_hash(maximum_signature_callback_block.clone())
        .await?;
    let maximum_signature_issued =
        maximum_signature_issued.expect("maximum signature response persists");
    assert_eq!(
        maximum_signature_issued.signature_status,
        SignatureStatus::Issued
    );
    assert_eq!(
        maximum_signature_issued
            .signature_response
            .as_deref()
            .expect("maximum callback response is stored")
            .len(),
        1024
    );
    storage_points.push((
        "max_signature_response_issued",
        contract_storage_usage_at(&administrator, maximum_signature_callback_block).await?,
    ));

    // These are observations, not fixed budgets: protocol/serialization updates may legitimately change the byte totals.
    eprintln!("NEAR_STORAGE_LIFECYCLE_BYTES:");
    for (index, (phase, bytes)) in storage_points.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .map(|previous_index| storage_points[previous_index].1)
            .unwrap_or(*bytes);
        eprintln!(
            "phase={phase} total_bytes={bytes} delta_from_previous_bytes={}",
            i128::from(*bytes) - i128::from(previous)
        );
    }

    Ok(())
}
