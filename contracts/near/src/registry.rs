use near_sdk::{near, AccountId};

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainReference {
    pub namespace: String,
    pub chain_id: String,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainCapabilities {
    pub chain_signatures: bool,
    pub contract_deploy: bool,
    pub eas: bool,
    pub safe: bool,
    pub baal: bool,
    pub intents_settlement: bool,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainConfig {
    pub reference: ChainReference,
    pub family: String,
    pub signing_scheme: String,
    pub capabilities: ChainCapabilities,
    pub enabled: bool,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub project_id: String,
    pub profile_hash: String,
    pub approved_chains: Vec<ChainReference>,
    pub active: bool,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignerBinding {
    pub project_id: String,
    pub chain: ChainReference,
    pub purpose: String,
    pub derivation_path: String,
    pub signing_scheme: String,
    pub derived_address: String,
    pub key_version: u32,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionPolicy {
    pub project_id: String,
    pub chain: ChainReference,
    pub version: String,
    pub allowed_operations: Vec<String>,
    pub allowed_targets: Vec<String>,
    pub max_value_base_units: String,
    pub operators: Vec<AccountId>,
    pub approvers: Vec<AccountId>,
    pub approval_threshold: u8,
    pub policy_hash: String,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GasPolicy {
    pub max_fee: String,
    pub max_priority_fee: String,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionRequest {
    pub protocol_version: String,
    pub request_id: String,
    pub project_id: String,
    pub chain_namespace: String,
    pub chain_id: String,
    pub operation: String,
    pub target: String,
    pub value_base_units: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_limit: Option<String>,
    pub calldata_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_payload_hash: Option<String>,
    pub nonce: String,
    pub gas_policy: GasPolicy,
    pub expires_at: String,
    pub policy_version: String,
    pub idempotency_key: String,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    AwaitingApproval,
    Approved,
    Expired,
    Cancelled,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionRequestRecord {
    pub request: ExecutionRequest,
    pub action_hash: String,
    pub policy_hash: String,
    pub proposer: AccountId,
    pub status: ExecutionStatus,
    pub approval_count: u8,
    pub expires_at_ns: u64,
}

pub(crate) fn validate_chain(reference: &ChainReference, family: &str, signing_scheme: &str) {
    assert!(
        !reference.namespace.is_empty()
            && reference.namespace.len() <= 64
            && reference
                .namespace
                .bytes()
                .enumerate()
                .all(|(index, byte)| {
                    if index == 0 {
                        byte.is_ascii_lowercase()
                    } else {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    }
                }),
        "invalid chain namespace"
    );
    assert!(
        !reference.chain_id.is_empty()
            && reference.chain_id.len() <= 128
            && reference
                .chain_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)),
        "invalid chain ID"
    );
    assert!(
        matches!(family, "evm" | "near" | "other"),
        "invalid chain family"
    );
    assert!(
        matches!(signing_scheme, "secp256k1" | "ed25519" | "other"),
        "invalid signing scheme"
    );

    if family == "evm" {
        assert_eq!(
            reference.namespace, "eip155",
            "EVM namespace must be eip155"
        );
        assert!(
            reference.chain_id.bytes().all(|byte| byte.is_ascii_digit())
                && !reference.chain_id.starts_with('0'),
            "EVM chain ID must be positive decimal"
        );
        assert_eq!(
            signing_scheme, "secp256k1",
            "EVM signing scheme must be secp256k1"
        );
    }

    if family == "near" {
        assert_eq!(reference.namespace, "near", "NEAR namespace must be near");
        assert!(
            matches!(signing_scheme, "ed25519" | "secp256k1"),
            "unsupported NEAR signing scheme"
        );
    }
}

pub(crate) fn is_valid_project_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

pub(crate) fn is_valid_hash(value: &str) -> bool {
    value.len() == 66
        && value.starts_with("0x")
        && value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn chain_key(reference: &ChainReference) -> String {
    format!("{}:{}", reference.namespace, reference.chain_id)
}

pub(crate) fn signer_scope_key(
    project_id: &str,
    reference: &ChainReference,
    purpose: &str,
) -> String {
    format!(
        "{project_id}:{}:{}:{purpose}",
        reference.namespace, reference.chain_id
    )
}

pub(crate) fn signer_binding_key(
    project_id: &str,
    reference: &ChainReference,
    purpose: &str,
    key_version: u32,
) -> String {
    format!(
        "{}:{key_version}",
        signer_scope_key(project_id, reference, purpose)
    )
}

pub(crate) fn expected_derivation_path(
    project_id: &str,
    reference: &ChainReference,
    purpose: &str,
) -> String {
    format!(
        "kokonut/v1/project/{project_id}/{}/{}/{purpose}",
        reference.namespace, reference.chain_id
    )
}

pub(crate) fn is_valid_signer_address(value: &str, family: &str) -> bool {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return false;
    }

    if family == "evm" {
        return value.len() == 42
            && value.starts_with("0x")
            && value[2..].bytes().all(|byte| byte.is_ascii_hexdigit());
    }

    true
}

pub(crate) fn execution_policy_scope_key(project_id: &str, reference: &ChainReference) -> String {
    format!(
        "{project_id}:{}:{}",
        reference.namespace, reference.chain_id
    )
}

pub(crate) fn execution_policy_key(
    project_id: &str,
    reference: &ChainReference,
    version: &str,
) -> String {
    format!(
        "{}:{version}",
        execution_policy_scope_key(project_id, reference)
    )
}

pub(crate) fn is_valid_policy_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'-' | b'_' | b'.'))
        })
}

pub(crate) fn is_valid_operation(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && value.len() <= 80
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

pub(crate) fn is_valid_base_units(value: &str) -> bool {
    const UINT256_MAX: &str =
        "115792089237316195423570985008687907853269984665640564039457584007913129639935";

    if value.is_empty()
        || value.len() > UINT256_MAX.len()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return false;
    }

    value.len() < UINT256_MAX.len() || value <= UINT256_MAX
}

pub(crate) fn base_units_lte(left: &str, right: &str) -> bool {
    is_valid_base_units(left)
        && is_valid_base_units(right)
        && (left.len() < right.len() || (left.len() == right.len() && left <= right))
}

pub(crate) fn parse_expires_at_ns(value: &str) -> Option<u64> {
    if value.len() > 64 {
        return None;
    }
    let timestamp =
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
            .ok()?
            .unix_timestamp_nanos();
    u64::try_from(timestamp).ok()
}

pub(crate) fn execution_approval_key(request_id: &str, approver: &AccountId) -> String {
    format!("{request_id}:{approver}")
}

pub(crate) fn hash_to_hex(hash: &[u8]) -> String {
    let mut output = String::with_capacity(2 + hash.len() * 2);
    output.push_str("0x");
    for byte in hash {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_execution_status_serializes_for_client_gate() {
        assert_eq!(
            near_sdk::serde_json::to_string(&ExecutionStatus::Approved)
                .expect("execution status is JSON serializable"),
            "\"Approved\""
        );
    }

    #[test]
    fn execution_request_json_example_deserializes_to_contract_input() {
        let request: ExecutionRequest = near_sdk::serde_json::from_str(include_str!(
            "../../../specs/examples/execution-request.json"
        ))
        .expect("schema example matches contract request type");

        assert_eq!(request.protocol_version, "1");
        assert_eq!(request.chain_namespace, "eip155");
        assert_eq!(request.gas_policy.max_fee, "1000000000");
        assert_eq!(request.gas_limit.as_deref(), Some("120000"));
        assert!(request.signing_payload_hash.is_some());
    }

    #[test]
    fn execution_request_fixture_action_hash_matches_contract_serialization() {
        near_sdk::testing_env!(near_sdk::test_utils::VMContextBuilder::new().build());
        let fixture: near_sdk::serde_json::Value = near_sdk::serde_json::from_str(include_str!(
            "../../../specs/examples/evm-signing-payload.json"
        ))
        .expect("signing payload fixture is JSON");
        let request: ExecutionRequest =
            near_sdk::serde_json::from_value(fixture["request"].clone())
                .expect("signing payload fixture request matches contract input");
        let serialized = near_sdk::serde_json::to_vec(&request)
            .expect("execution request serializes deterministically");
        let actual = hash_to_hex(&near_sdk::env::sha256(&serialized));

        assert_eq!(
            actual,
            fixture["expected_action_hash"]
                .as_str()
                .expect("fixture action hash is a string")
        );
    }

    #[test]
    fn base_unit_values_are_canonical_uint256_decimals() {
        assert!(is_valid_base_units("0"));
        assert!(is_valid_base_units(
            "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        ));
        assert!(!is_valid_base_units(""));
        assert!(!is_valid_base_units("00"));
        assert!(!is_valid_base_units("1.0"));
        assert!(!is_valid_base_units(
            "115792089237316195423570985008687907853269984665640564039457584007913129639936"
        ));
    }

    #[test]
    fn policy_version_and_operation_identifiers_are_constrained() {
        assert!(is_valid_policy_version("policy-v1.0"));
        assert!(!is_valid_policy_version(""));
        assert!(!is_valid_policy_version("-policy"));
        assert!(is_valid_operation("deploy_project_component"));
        assert!(!is_valid_operation("DeployProject"));
        assert!(!is_valid_operation("deploy-project"));
    }

    #[test]
    fn base_unit_comparison_is_numeric_for_canonical_decimal_strings() {
        assert!(base_units_lte("0", "0"));
        assert!(base_units_lte("9", "10"));
        assert!(base_units_lte("100", "100"));
        assert!(!base_units_lte("10", "9"));
        assert!(!base_units_lte("01", "10"));
    }

    #[test]
    fn expiration_parser_accepts_rfc3339_and_rejects_invalid_or_pre_epoch_values() {
        assert!(parse_expires_at_ns("2035-01-01T00:00:00Z").is_some());
        assert!(parse_expires_at_ns("2035-01-01T01:00:00+01:00").is_some());
        assert!(parse_expires_at_ns("not-a-timestamp").is_none());
        assert!(parse_expires_at_ns("1969-12-31T23:59:59Z").is_none());
    }

    #[test]
    fn approval_keys_include_request_and_approver() {
        let alice = "alice.near".parse().unwrap();
        assert_eq!(
            execution_approval_key("request-1", &alice),
            "request-1:alice.near"
        );
    }

    #[test]
    fn policy_keys_are_scoped_by_project_chain_and_version() {
        let first_chain = ChainReference {
            namespace: "eip155".to_owned(),
            chain_id: "1".to_owned(),
        };
        let second_chain = ChainReference {
            namespace: "eip155".to_owned(),
            chain_id: "8453".to_owned(),
        };

        let first = execution_policy_key("project-a", &first_chain, "v1");
        let other_chain = execution_policy_key("project-a", &second_chain, "v1");
        let other_version = execution_policy_key("project-a", &first_chain, "v2");

        assert_ne!(first, other_chain);
        assert_ne!(first, other_version);
    }

    #[test]
    fn hash_to_hex_emits_lowercase_prefixed_hex() {
        assert_eq!(hash_to_hex(&[0x00, 0xab, 0xff]), "0x00abff");
    }
}
