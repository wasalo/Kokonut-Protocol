use near_sdk::near;

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
