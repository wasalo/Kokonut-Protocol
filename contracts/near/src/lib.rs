mod registry;

use near_sdk::store::LookupMap;
use near_sdk::{env, near, AccountId, PanicOnDefault};
pub use registry::{ChainCapabilities, ChainConfig, ChainReference, Project};
use std::collections::HashSet;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct KokonutProtocolHub {
    administrator: AccountId,
    paused: bool,
    chains: LookupMap<String, ChainConfig>,
    projects: LookupMap<String, Project>,
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
}
