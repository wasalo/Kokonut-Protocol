use kokonut_protocol_near::{ChainCapabilities, ChainConfig, ChainReference};
use near_kit::Near;
use near_sandbox::config::{DEFAULT_GENESIS_ACCOUNT, DEFAULT_GENESIS_ACCOUNT_PRIVATE_KEY};
use near_sandbox::{GenesisAccount, Sandbox, SandboxConfig};
use near_sdk::serde_json::json;
use std::fs;
use std::path::Path;

const SANDBOX_VERSION: &str = include_str!("../.near-sandbox-version");
const CONTRACT_ACCOUNT: &str = "sandbox";

#[tokio::test]
async fn deployed_hub_persists_admin_changes_and_rejects_non_admin_mutation(
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
    let mut config = SandboxConfig::default();
    config.additional_accounts.push(outsider.clone());

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
