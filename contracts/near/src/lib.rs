mod registry;

use near_sdk::store::LookupMap;
use near_sdk::{env, near, AccountId, PanicOnDefault};
pub use registry::{
    ChainCapabilities, ChainConfig, ChainReference, ExecutionPolicy, Project, SignerBinding,
};
use std::collections::HashSet;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct KokonutProtocolHub {
    administrator: AccountId,
    paused: bool,
    chains: LookupMap<String, ChainConfig>,
    projects: LookupMap<String, Project>,
    signer_bindings: LookupMap<String, SignerBinding>,
    active_signers: LookupMap<String, u32>,
    execution_policies: LookupMap<String, ExecutionPolicy>,
    active_execution_policies: LookupMap<String, String>,
}

#[near]
impl KokonutProtocolHub {
    #[init]
    pub fn new(administrator: AccountId) -> Self {
        assert!(
            !administrator.as_str().is_empty(),
            "administrator is required"
        );
        Self {
            administrator,
            paused: false,
            chains: LookupMap::new(vec![0x01]),
            projects: LookupMap::new(vec![0x02]),
            signer_bindings: LookupMap::new(vec![0x03]),
            active_signers: LookupMap::new(vec![0x04]),
            execution_policies: LookupMap::new(vec![0x05]),
            active_execution_policies: LookupMap::new(vec![0x06]),
        }
    }

    pub fn register_chain(
        &mut self,
        namespace: String,
        chain_id: String,
        family: String,
        signing_scheme: String,
        capabilities: ChainCapabilities,
        enabled: bool,
    ) {
        self.assert_administrator();
        let reference = ChainReference {
            namespace,
            chain_id,
        };
        registry::validate_chain(&reference, &family, &signing_scheme);
        let key = registry::chain_key(&reference);
        assert!(self.chains.get(&key).is_none(), "chain already registered");

        let config = ChainConfig {
            reference: reference.clone(),
            family,
            signing_scheme,
            capabilities,
            enabled,
        };
        self.chains.insert(key, config);
        self.emit_event(
            "chain_registered",
            near_sdk::serde_json::json!({ "chain": reference, "enabled": enabled }),
        );
    }

    pub fn set_chain_enabled(&mut self, namespace: String, chain_id: String, enabled: bool) {
        self.assert_administrator();
        let reference = ChainReference {
            namespace,
            chain_id,
        };
        let key = registry::chain_key(&reference);
        let mut config = self
            .chains
            .get(&key)
            .cloned()
            .unwrap_or_else(|| env::panic_str("chain not registered"));
        config.enabled = enabled;
        self.chains.insert(key, config);
        self.emit_event(
            "chain_status_changed",
            near_sdk::serde_json::json!({ "chain": reference, "enabled": enabled }),
        );
    }

    pub fn register_project(
        &mut self,
        project_id: String,
        profile_hash: String,
        approved_chains: Vec<ChainReference>,
    ) {
        self.assert_administrator();
        assert!(
            registry::is_valid_project_id(&project_id),
            "invalid project ID"
        );
        assert!(
            registry::is_valid_hash(&profile_hash),
            "invalid profile hash"
        );
        assert!(
            !approved_chains.is_empty(),
            "project must approve at least one chain"
        );
        assert!(
            self.projects.get(&project_id).is_none(),
            "project already registered"
        );

        let mut seen = HashSet::new();
        for reference in &approved_chains {
            assert!(
                seen.insert(registry::chain_key(reference)),
                "duplicate approved chain"
            );
            let chain = self
                .chains
                .get(&registry::chain_key(reference))
                .unwrap_or_else(|| env::panic_str("approved chain is not registered"));
            assert!(chain.enabled, "approved chain is disabled");
        }

        let project = Project {
            project_id: project_id.clone(),
            profile_hash: profile_hash.clone(),
            approved_chains: approved_chains.clone(),
            active: true,
        };
        self.projects.insert(project_id.clone(), project);
        self.emit_event(
            "project_registered",
            near_sdk::serde_json::json!({
                "project_id": project_id,
                "profile_hash": profile_hash,
                "approved_chains": approved_chains
            }),
        );
    }

    pub fn set_project_status(&mut self, project_id: String, active: bool) {
        self.assert_administrator();
        let mut project = self
            .projects
            .get(&project_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("project not registered"));
        project.active = active;
        self.projects.insert(project_id.clone(), project);
        self.emit_event(
            "project_status_changed",
            near_sdk::serde_json::json!({ "project_id": project_id, "active": active }),
        );
    }

    pub fn configure_signer(
        &mut self,
        project_id: String,
        namespace: String,
        chain_id: String,
        purpose: String,
        derivation_path: String,
        signing_scheme: String,
        derived_address: String,
        key_version: u32,
    ) {
        self.assert_administrator();
        assert!(
            registry::is_valid_project_id(&project_id),
            "invalid project ID"
        );
        assert!(
            matches!(purpose.as_str(), "deployer" | "operator" | "settlement"),
            "unsupported signer purpose"
        );
        assert!(key_version > 0, "key version must be positive");

        let reference = ChainReference {
            namespace,
            chain_id,
        };
        let project = self
            .projects
            .get(&project_id)
            .unwrap_or_else(|| env::panic_str("project not registered"));
        assert!(project.active, "project is inactive");
        assert!(
            project.approved_chains.contains(&reference),
            "chain is not approved for project"
        );
        let chain = self
            .chains
            .get(&registry::chain_key(&reference))
            .unwrap_or_else(|| env::panic_str("chain not registered"));
        assert!(chain.enabled, "chain is disabled");
        assert_eq!(
            signing_scheme, chain.signing_scheme,
            "signing scheme does not match chain policy"
        );
        assert_eq!(
            derivation_path,
            registry::expected_derivation_path(&project_id, &reference, &purpose),
            "derivation path must isolate project, chain, and purpose"
        );
        assert!(
            registry::is_valid_signer_address(&derived_address, &chain.family),
            "invalid derived address"
        );

        let binding_key =
            registry::signer_binding_key(&project_id, &reference, &purpose, key_version);
        assert!(
            self.signer_bindings.get(&binding_key).is_none(),
            "signer key version already registered"
        );
        let binding = SignerBinding {
            project_id: project_id.clone(),
            chain: reference.clone(),
            purpose: purpose.clone(),
            derivation_path,
            signing_scheme,
            derived_address: derived_address.clone(),
            key_version,
        };
        self.signer_bindings.insert(binding_key, binding);
        self.emit_event(
            "signer_binding_registered",
            near_sdk::serde_json::json!({
                "project_id": project_id,
                "chain": reference,
                "purpose": purpose,
                "derived_address": derived_address,
                "key_version": key_version
            }),
        );
    }

    pub fn activate_signer(
        &mut self,
        project_id: String,
        namespace: String,
        chain_id: String,
        purpose: String,
        key_version: u32,
    ) {
        self.assert_administrator();
        let reference = ChainReference {
            namespace,
            chain_id,
        };
        let project = self
            .projects
            .get(&project_id)
            .unwrap_or_else(|| env::panic_str("project not registered"));
        assert!(project.active, "project is inactive");
        assert!(
            project.approved_chains.contains(&reference),
            "chain is not approved for project"
        );
        let chain = self
            .chains
            .get(&registry::chain_key(&reference))
            .unwrap_or_else(|| env::panic_str("chain not registered"));
        assert!(chain.enabled, "chain is disabled");

        let binding_key =
            registry::signer_binding_key(&project_id, &reference, &purpose, key_version);
        assert!(
            self.signer_bindings.get(&binding_key).is_some(),
            "signer binding not registered"
        );
        let scope_key = registry::signer_scope_key(&project_id, &reference, &purpose);
        self.active_signers.insert(scope_key, key_version);
        self.emit_event(
            "signer_binding_activated",
            near_sdk::serde_json::json!({
                "project_id": project_id,
                "chain": reference,
                "purpose": purpose,
                "key_version": key_version
            }),
        );
    }

    pub fn get_signer_binding(
        &self,
        project_id: String,
        namespace: String,
        chain_id: String,
        purpose: String,
        key_version: u32,
    ) -> Option<SignerBinding> {
        let reference = ChainReference {
            namespace,
            chain_id,
        };
        self.signer_bindings
            .get(&registry::signer_binding_key(
                &project_id,
                &reference,
                &purpose,
                key_version,
            ))
            .cloned()
    }

    pub fn get_active_signer(
        &self,
        project_id: String,
        namespace: String,
        chain_id: String,
        purpose: String,
    ) -> Option<SignerBinding> {
        let reference = ChainReference {
            namespace,
            chain_id,
        };
        let scope_key = registry::signer_scope_key(&project_id, &reference, &purpose);
        let key_version = self.active_signers.get(&scope_key).copied()?;
        self.signer_bindings
            .get(&registry::signer_binding_key(
                &project_id,
                &reference,
                &purpose,
                key_version,
            ))
            .cloned()
    }

    pub fn configure_execution_policy(
        &mut self,
        project_id: String,
        namespace: String,
        chain_id: String,
        version: String,
        allowed_operations: Vec<String>,
        allowed_targets: Vec<String>,
        max_value_base_units: String,
        approvers: Vec<AccountId>,
        approval_threshold: u8,
    ) {
        self.assert_administrator();
        assert!(
            registry::is_valid_project_id(&project_id),
            "invalid project ID"
        );
        assert!(
            registry::is_valid_policy_version(&version),
            "invalid policy version"
        );
        assert!(
            registry::is_valid_base_units(&max_value_base_units),
            "max value must be a canonical uint256 decimal"
        );
        assert!(
            !allowed_operations.is_empty(),
            "policy requires an operation"
        );
        assert!(!allowed_targets.is_empty(), "policy requires a target");
        assert!(!approvers.is_empty(), "policy requires an approver");
        assert!(
            approval_threshold > 0 && usize::from(approval_threshold) <= approvers.len(),
            "approval threshold must be within approver count"
        );
        assert!(
            allowed_operations
                .iter()
                .all(|value| registry::is_valid_operation(value)),
            "invalid operation identifier"
        );
        assert_eq!(
            allowed_operations.iter().collect::<HashSet<_>>().len(),
            allowed_operations.len(),
            "duplicate allowed operation"
        );
        assert_eq!(
            approvers.iter().collect::<HashSet<_>>().len(),
            approvers.len(),
            "duplicate approver"
        );

        let reference = ChainReference {
            namespace,
            chain_id,
        };
        let project = self
            .projects
            .get(&project_id)
            .unwrap_or_else(|| env::panic_str("project not registered"));
        assert!(project.active, "project is inactive");
        assert!(
            project.approved_chains.contains(&reference),
            "chain is not approved for project"
        );
        let chain = self
            .chains
            .get(&registry::chain_key(&reference))
            .unwrap_or_else(|| env::panic_str("chain not registered"));
        assert!(chain.enabled, "chain is disabled");
        assert!(
            allowed_targets
                .iter()
                .all(|target| registry::is_valid_signer_address(target, &chain.family)),
            "invalid target for chain family"
        );
        let normalized_targets: HashSet<String> = allowed_targets
            .iter()
            .map(|target| {
                if chain.family == "evm" {
                    target.to_ascii_lowercase()
                } else {
                    target.clone()
                }
            })
            .collect();
        assert_eq!(
            normalized_targets.len(),
            allowed_targets.len(),
            "duplicate allowed target"
        );

        let policy_key = registry::execution_policy_key(&project_id, &reference, &version);
        assert!(
            self.execution_policies.get(&policy_key).is_none(),
            "policy version already registered"
        );
        let hash_input = near_sdk::serde_json::to_vec(&(
            &project_id,
            &reference,
            &version,
            &allowed_operations,
            &allowed_targets,
            &max_value_base_units,
            &approvers,
            approval_threshold,
        ))
        .unwrap_or_else(|_| env::panic_str("failed to encode policy commitment"));
        let policy_hash = registry::hash_to_hex(&env::sha256(&hash_input));
        let policy = ExecutionPolicy {
            project_id: project_id.clone(),
            chain: reference.clone(),
            version: version.clone(),
            allowed_operations,
            allowed_targets,
            max_value_base_units,
            approvers,
            approval_threshold,
            policy_hash: policy_hash.clone(),
        };
        self.execution_policies.insert(policy_key, policy);
        self.active_execution_policies.insert(
            registry::execution_policy_scope_key(&project_id, &reference),
            version.clone(),
        );
        self.emit_event(
            "execution_policy_activated",
            near_sdk::serde_json::json!({
                "project_id": project_id,
                "chain": reference,
                "version": version,
                "policy_hash": policy_hash,
                "approval_threshold": approval_threshold
            }),
        );
    }

    pub fn get_execution_policy(
        &self,
        project_id: String,
        namespace: String,
        chain_id: String,
        version: String,
    ) -> Option<ExecutionPolicy> {
        let reference = ChainReference {
            namespace,
            chain_id,
        };
        self.execution_policies
            .get(&registry::execution_policy_key(
                &project_id,
                &reference,
                &version,
            ))
            .cloned()
    }

    pub fn get_active_execution_policy(
        &self,
        project_id: String,
        namespace: String,
        chain_id: String,
    ) -> Option<ExecutionPolicy> {
        let reference = ChainReference {
            namespace,
            chain_id,
        };
        let scope_key = registry::execution_policy_scope_key(&project_id, &reference);
        let version = self.active_execution_policies.get(&scope_key)?;
        self.execution_policies
            .get(&registry::execution_policy_key(
                &project_id,
                &reference,
                version,
            ))
            .cloned()
    }

    pub fn pause(&mut self) {
        self.assert_administrator();
        self.paused = true;
        self.emit_event("protocol_paused", near_sdk::serde_json::json!({}));
    }

    pub fn unpause(&mut self) {
        self.assert_administrator();
        self.paused = false;
        self.emit_event("protocol_unpaused", near_sdk::serde_json::json!({}));
    }

    pub fn get_chain(&self, namespace: String, chain_id: String) -> Option<ChainConfig> {
        let reference = ChainReference {
            namespace,
            chain_id,
        };
        self.chains.get(&registry::chain_key(&reference)).cloned()
    }

    pub fn get_project(&self, project_id: String) -> Option<Project> {
        self.projects.get(&project_id).cloned()
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    fn assert_administrator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.administrator,
            "administrator only"
        );
    }

    fn emit_event(&self, event: &str, data: near_sdk::serde_json::Value) {
        env::log_str(&format!(
            "EVENT_JSON:{}",
            near_sdk::serde_json::json!({
                "standard": "kokonut_protocol",
                "version": "1.0.0",
                "event": event,
                "data": data
            })
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, AccountId};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    const ADMIN: &str = "governance.kokonut.near";
    const STRANGER: &str = "stranger.near";
    const PROJECT_ID: &str = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c3b";
    const PROFILE_HASH: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn account(value: &str) -> AccountId {
        value.parse().expect("valid account ID")
    }

    fn set_predecessor(value: &str) {
        testing_env!(VMContextBuilder::new()
            .predecessor_account_id(account(value))
            .build());
    }

    fn initialized_hub() -> KokonutProtocolHub {
        set_predecessor(ADMIN);
        KokonutProtocolHub::new(account(ADMIN))
    }

    fn register_evm_chain(hub: &mut KokonutProtocolHub, enabled: bool) {
        hub.register_chain(
            "eip155".to_owned(),
            "84532".to_owned(),
            "evm".to_owned(),
            "secp256k1".to_owned(),
            ChainCapabilities {
                chain_signatures: false,
                contract_deploy: true,
                eas: false,
                safe: true,
                baal: false,
                intents_settlement: false,
            },
            enabled,
        );
    }

    #[test]
    fn administrator_can_register_chain_and_project() {
        let mut hub = initialized_hub();
        register_evm_chain(&mut hub, true);

        hub.register_project(
            PROJECT_ID.to_owned(),
            PROFILE_HASH.to_owned(),
            vec![ChainReference {
                namespace: "eip155".to_owned(),
                chain_id: "84532".to_owned(),
            }],
        );

        let project = hub
            .get_project(PROJECT_ID.to_owned())
            .expect("registered project");
        assert_eq!(project.profile_hash, PROFILE_HASH);
        assert!(project.active);
        assert_eq!(project.approved_chains.len(), 1);
    }

    #[test]
    fn administrator_can_suspend_project_and_toggle_pause() {
        let mut hub = initialized_hub();
        register_evm_chain(&mut hub, true);
        hub.register_project(
            PROJECT_ID.to_owned(),
            PROFILE_HASH.to_owned(),
            vec![ChainReference {
                namespace: "eip155".to_owned(),
                chain_id: "84532".to_owned(),
            }],
        );

        hub.set_project_status(PROJECT_ID.to_owned(), false);
        assert!(!hub.get_project(PROJECT_ID.to_owned()).unwrap().active);
        hub.pause();
        assert!(hub.is_paused());
        hub.unpause();
        assert!(!hub.is_paused());
    }

    #[test]
    fn non_administrator_cannot_register_chain() {
        let mut hub = initialized_hub();
        set_predecessor(STRANGER);

        let result = catch_unwind(AssertUnwindSafe(|| register_evm_chain(&mut hub, true)));

        assert!(result.is_err());
    }

    #[test]
    fn project_cannot_approve_disabled_chain() {
        let mut hub = initialized_hub();
        register_evm_chain(&mut hub, false);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.register_project(
                PROJECT_ID.to_owned(),
                PROFILE_HASH.to_owned(),
                vec![ChainReference {
                    namespace: "eip155".to_owned(),
                    chain_id: "84532".to_owned(),
                }],
            )
        }));

        assert!(result.is_err());
        assert!(hub.get_project(PROJECT_ID.to_owned()).is_none());
    }

    #[test]
    fn project_profile_hash_must_be_32_byte_hex() {
        let mut hub = initialized_hub();
        register_evm_chain(&mut hub, true);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.register_project(
                PROJECT_ID.to_owned(),
                "0x1234".to_owned(),
                vec![ChainReference {
                    namespace: "eip155".to_owned(),
                    chain_id: "84532".to_owned(),
                }],
            )
        }));

        assert!(result.is_err());
    }

    fn registered_project(hub: &mut KokonutProtocolHub) {
        register_evm_chain(hub, true);
        hub.register_project(
            PROJECT_ID.to_owned(),
            PROFILE_HASH.to_owned(),
            vec![ChainReference {
                namespace: "eip155".to_owned(),
                chain_id: "84532".to_owned(),
            }],
        );
    }

    fn configure_signer(hub: &mut KokonutProtocolHub, key_version: u32, address: &str) {
        hub.configure_signer(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            "deployer".to_owned(),
            format!("kokonut/v1/project/{PROJECT_ID}/eip155/84532/deployer"),
            "secp256k1".to_owned(),
            address.to_owned(),
            key_version,
        );
    }

    fn configure_execution_policy(hub: &mut KokonutProtocolHub, version: &str) {
        hub.configure_execution_policy(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            version.to_owned(),
            vec!["deploy_project_component".to_owned()],
            vec!["0x1234567890abcdef1234567890abcdef12345678".to_owned()],
            "1000000".to_owned(),
            vec!["alice.near".parse().unwrap(), "bob.near".parse().unwrap()],
            2,
        );
    }

    #[test]
    fn administrator_can_create_and_activate_a_versioned_execution_policy() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");

        let policy = hub
            .get_active_execution_policy(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
            )
            .expect("configured policy is active");

        assert_eq!(policy.version, "policy-v1");
        assert_eq!(policy.approval_threshold, 2);
        assert_eq!(policy.approvers.len(), 2);
        assert_eq!(policy.max_value_base_units, "1000000");
        assert!(registry::is_valid_hash(&policy.policy_hash));
    }

    #[test]
    fn execution_policy_rejects_threshold_larger_than_approver_set() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.configure_execution_policy(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
                "policy-invalid-threshold".to_owned(),
                vec!["deploy_project_component".to_owned()],
                vec!["0x1234567890abcdef1234567890abcdef12345678".to_owned()],
                "1000000".to_owned(),
                vec![account("alice.near")],
                2,
            )
        }));

        assert!(result.is_err());
    }

    #[test]
    fn execution_policy_rejects_invalid_evm_target() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.configure_execution_policy(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
                "policy-invalid-target".to_owned(),
                vec!["deploy_project_component".to_owned()],
                vec!["not-an-evm-address".to_owned()],
                "1000000".to_owned(),
                vec![account("alice.near")],
                1,
            )
        }));

        assert!(result.is_err());
    }

    #[test]
    fn execution_policy_versions_are_immutable_and_new_versions_activate() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        let original = hub
            .get_execution_policy(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
                "policy-v1".to_owned(),
            )
            .expect("first policy version remains stored");
        configure_execution_policy(&mut hub, "policy-v2");
        let active = hub
            .get_active_execution_policy(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
            )
            .expect("second policy version is active");
        let reused_version = catch_unwind(AssertUnwindSafe(|| {
            configure_execution_policy(&mut hub, "policy-v1")
        }));

        assert!(reused_version.is_err());
        assert_eq!(original.version, "policy-v1");
        assert_eq!(active.version, "policy-v2");
        assert_ne!(original.policy_hash, active.policy_hash);
    }

    #[test]
    fn non_administrator_cannot_configure_execution_policy() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        set_predecessor(STRANGER);

        let result = catch_unwind(AssertUnwindSafe(|| {
            configure_execution_policy(&mut hub, "policy-v1")
        }));

        assert!(result.is_err());
        set_predecessor(ADMIN);
        assert!(hub
            .get_active_execution_policy(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
            )
            .is_none());
    }

    #[test]
    fn signer_binding_must_use_project_chain_and_purpose_specific_path() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.configure_signer(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
                "deployer".to_owned(),
                "kokonut/v1/project/another-project/eip155/84532/deployer".to_owned(),
                "secp256k1".to_owned(),
                "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
                1,
            )
        }));

        assert!(result.is_err());
        assert!(hub
            .get_signer_binding(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
                "deployer".to_owned(),
                1,
            )
            .is_none());
    }

    #[test]
    fn non_administrator_cannot_configure_signer() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        set_predecessor(STRANGER);

        let result = catch_unwind(AssertUnwindSafe(|| {
            configure_signer(&mut hub, 1, "0x1234567890abcdef1234567890abcdef12345678")
        }));

        assert!(result.is_err());
        set_predecessor(ADMIN);
        assert!(hub
            .get_signer_binding(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
                "deployer".to_owned(),
                1,
            )
            .is_none());
    }

    #[test]
    fn signer_binding_rejects_unapproved_chain_and_malformed_evm_address() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        hub.register_chain(
            "eip155".to_owned(),
            "1".to_owned(),
            "evm".to_owned(),
            "secp256k1".to_owned(),
            ChainCapabilities {
                chain_signatures: false,
                contract_deploy: true,
                eas: false,
                safe: true,
                baal: false,
                intents_settlement: false,
            },
            true,
        );

        let wrong_chain = catch_unwind(AssertUnwindSafe(|| {
            hub.configure_signer(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "1".to_owned(),
                "deployer".to_owned(),
                format!("kokonut/v1/project/{PROJECT_ID}/eip155/1/deployer"),
                "secp256k1".to_owned(),
                "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
                1,
            )
        }));
        let malformed_address =
            catch_unwind(AssertUnwindSafe(|| configure_signer(&mut hub, 1, "0x1234")));

        assert!(wrong_chain.is_err());
        assert!(malformed_address.is_err());
    }

    #[test]
    fn signer_rotation_preserves_old_binding_and_activates_new_version() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        let first_address = "0x1234567890abcdef1234567890abcdef12345678";
        let second_address = "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd";

        configure_signer(&mut hub, 1, first_address);
        hub.activate_signer(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            "deployer".to_owned(),
            1,
        );
        configure_signer(&mut hub, 2, second_address);
        hub.activate_signer(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            "deployer".to_owned(),
            2,
        );

        let old = hub
            .get_signer_binding(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
                "deployer".to_owned(),
                1,
            )
            .expect("old binding remains in history");
        let active = hub
            .get_active_signer(
                PROJECT_ID.to_owned(),
                "eip155".to_owned(),
                "84532".to_owned(),
                "deployer".to_owned(),
            )
            .expect("new binding is active");

        assert_eq!(old.derived_address, first_address);
        assert_eq!(active.key_version, 2);
        assert_eq!(active.derived_address, second_address);
    }
}
