mod registry;

use near_sdk::store::LookupMap;
use near_sdk::{env, near, AccountId, Gas, NearToken, PanicOnDefault, Promise};
pub use registry::{
    ChainCapabilities, ChainConfig, ChainReference, ChainSignaturesConfig, ExecutionPolicy,
    ExecutionRequest, ExecutionRequestRecord, ExecutionStatus, GasPolicy, Project, SignatureStatus,
    SignerBinding,
};
use std::collections::HashSet;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct KokonutProtocolHub {
    administrator: AccountId,
    paused: bool,
    chain_signatures: Option<ChainSignaturesConfig>,
    chains: LookupMap<String, ChainConfig>,
    projects: LookupMap<String, Project>,
    signer_bindings: LookupMap<String, SignerBinding>,
    active_signers: LookupMap<String, u32>,
    execution_policies: LookupMap<String, ExecutionPolicy>,
    active_execution_policies: LookupMap<String, String>,
    execution_requests: LookupMap<String, ExecutionRequestRecord>,
    idempotency_requests: LookupMap<String, String>,
    execution_approvals: LookupMap<String, bool>,
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
            chain_signatures: None,
            chains: LookupMap::new(vec![0x01]),
            projects: LookupMap::new(vec![0x02]),
            signer_bindings: LookupMap::new(vec![0x03]),
            active_signers: LookupMap::new(vec![0x04]),
            execution_policies: LookupMap::new(vec![0x05]),
            active_execution_policies: LookupMap::new(vec![0x06]),
            execution_requests: LookupMap::new(vec![0x07]),
            idempotency_requests: LookupMap::new(vec![0x08]),
            execution_approvals: LookupMap::new(vec![0x09]),
        }
    }

    pub fn configure_chain_signatures(&mut self, contract_id: AccountId, public_key: String) {
        self.assert_administrator();
        assert!(
            contract_id.as_str().ends_with(".testnet"),
            "testnet Chain Signatures contract required"
        );
        let encoded_public_key = public_key.strip_prefix("secp256k1:").unwrap_or("");
        assert!(
            !encoded_public_key.is_empty()
                && encoded_public_key.len() <= 128
                && encoded_public_key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric()),
            "valid secp256k1 public key required"
        );
        let configuration = ChainSignaturesConfig {
            contract_id: contract_id.clone(),
            public_key: public_key.clone(),
        };
        self.chain_signatures = Some(configuration);
        self.emit_event(
            "chain_signatures_configured",
            near_sdk::serde_json::json!({
                "contract_id": contract_id,
                "public_key": public_key
            }),
        );
    }

    pub fn get_chain_signatures_config(&self) -> Option<ChainSignaturesConfig> {
        self.chain_signatures.clone()
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
        operators: Vec<AccountId>,
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
        assert!(
            !operators.is_empty(),
            "policy requires an execution operator"
        );
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
            operators.iter().collect::<HashSet<_>>().len(),
            operators.len(),
            "duplicate execution operator"
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
            &operators,
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
            operators,
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

    pub fn propose_execution(&mut self, request: ExecutionRequest) {
        assert!(!self.paused, "protocol is paused");
        assert_eq!(
            request.protocol_version, "1",
            "unsupported protocol version"
        );
        assert!(
            registry::is_valid_project_id(&request.request_id),
            "invalid request ID"
        );
        assert!(
            registry::is_valid_project_id(&request.project_id),
            "invalid project ID"
        );
        assert!(
            registry::is_valid_operation(&request.operation),
            "invalid operation identifier"
        );
        assert!(
            registry::is_valid_base_units(&request.value_base_units),
            "invalid value amount"
        );
        assert!(
            registry::is_valid_base_units(&request.nonce),
            "invalid nonce"
        );
        assert!(
            registry::is_valid_base_units(&request.gas_policy.max_fee)
                && registry::is_valid_base_units(&request.gas_policy.max_priority_fee),
            "invalid gas fee amount"
        );
        assert!(
            registry::base_units_lte(
                &request.gas_policy.max_priority_fee,
                &request.gas_policy.max_fee
            ),
            "priority fee exceeds max fee"
        );
        assert!(
            registry::is_valid_hash(&request.calldata_hash),
            "invalid calldata hash"
        );
        assert!(
            !request.idempotency_key.is_empty()
                && request.idempotency_key.len() <= 256
                && request
                    .idempotency_key
                    .bytes()
                    .all(|byte| (0x20..=0x7e).contains(&byte)),
            "invalid idempotency key"
        );
        let expires_at_ns = registry::parse_expires_at_ns(&request.expires_at)
            .unwrap_or_else(|| env::panic_str("invalid RFC 3339 expiration"));
        assert!(
            expires_at_ns > env::block_timestamp(),
            "execution request is expired"
        );
        assert!(
            self.execution_requests.get(&request.request_id).is_none(),
            "request ID already used"
        );
        assert!(
            self.idempotency_requests
                .get(&request.idempotency_key)
                .is_none(),
            "idempotency key already used"
        );

        let reference = ChainReference {
            namespace: request.chain_namespace.clone(),
            chain_id: request.chain_id.clone(),
        };
        let project = self
            .projects
            .get(&request.project_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("project not registered"));
        assert!(project.active, "project is inactive");
        assert!(
            project.approved_chains.contains(&reference),
            "chain is not approved for project"
        );
        let chain = self
            .chains
            .get(&registry::chain_key(&reference))
            .cloned()
            .unwrap_or_else(|| env::panic_str("chain not registered"));
        assert!(chain.enabled, "chain is disabled");
        assert!(
            registry::is_valid_signer_address(&request.target, &chain.family),
            "invalid target for chain family"
        );
        if chain.family == "evm" {
            let gas_limit = request
                .gas_limit
                .as_deref()
                .unwrap_or_else(|| env::panic_str("EVM request is missing gas limit"));
            assert!(
                registry::is_valid_base_units(gas_limit) && gas_limit != "0",
                "invalid EVM gas limit"
            );
            let signing_payload_hash = request
                .signing_payload_hash
                .as_deref()
                .unwrap_or_else(|| env::panic_str("EVM request is missing signing payload hash"));
            assert!(
                registry::is_valid_hash(signing_payload_hash),
                "invalid EVM signing payload hash"
            );
            let _ = request
                .signer_key_version
                .unwrap_or_else(|| env::panic_str("EVM request is missing signer key version"));
            let expected_effect_hash = request
                .expected_effect_hash
                .as_deref()
                .unwrap_or_else(|| env::panic_str("EVM request is missing expected effect hash"));
            assert!(
                registry::is_valid_hash(expected_effect_hash),
                "invalid expected effect hash"
            );
        } else {
            assert!(
                request.gas_limit.is_none()
                    && request.signing_payload_hash.is_none()
                    && request.signer_key_version.is_none()
                    && request.expected_effect_hash.is_none(),
                "EVM signing fields are not applicable to this chain family"
            );
        }

        let policy = self
            .get_active_execution_policy(
                request.project_id.clone(),
                request.chain_namespace.clone(),
                request.chain_id.clone(),
            )
            .unwrap_or_else(|| env::panic_str("execution policy not configured"));
        assert_eq!(
            request.policy_version, policy.version,
            "request policy version is not active"
        );
        let predecessor = env::predecessor_account_id();
        assert!(
            policy.operators.contains(&predecessor),
            "caller is not an execution operator"
        );
        assert!(
            policy.allowed_operations.contains(&request.operation),
            "operation is not allowed by policy"
        );
        assert!(
            policy.allowed_targets.iter().any(|target| {
                if chain.family == "evm" {
                    target.eq_ignore_ascii_case(&request.target)
                } else {
                    target == &request.target
                }
            }),
            "target is not allowed by policy"
        );
        assert!(
            registry::base_units_lte(&request.value_base_units, &policy.max_value_base_units),
            "value exceeds policy limit"
        );

        let serialized = near_sdk::serde_json::to_vec(&request)
            .unwrap_or_else(|_| env::panic_str("failed to encode execution request"));
        let action_hash = registry::hash_to_hex(&env::sha256(&serialized));
        let record = ExecutionRequestRecord {
            request: request.clone(),
            action_hash: action_hash.clone(),
            policy_hash: policy.policy_hash.clone(),
            proposer: predecessor,
            status: ExecutionStatus::AwaitingApproval,
            signature_status: SignatureStatus::NotRequested,
            signature_public_key: None,
            signature_response: None,
            approval_count: 0,
            expires_at_ns,
        };
        self.execution_requests
            .insert(request.request_id.clone(), record);
        self.idempotency_requests
            .insert(request.idempotency_key.clone(), request.request_id.clone());
        self.emit_event(
            "execution_proposed",
            near_sdk::serde_json::json!({
                "request_id": request.request_id,
                "project_id": request.project_id,
                "chain_namespace": request.chain_namespace,
                "chain_id": request.chain_id,
                "action_hash": action_hash,
                "policy_hash": policy.policy_hash
            }),
        );
    }

    pub fn approve_execution(&mut self, request_id: String, expected_action_hash: String) {
        let mut record = self
            .execution_requests
            .get(&request_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("execution request not found"));
        assert_eq!(
            record.status,
            ExecutionStatus::AwaitingApproval,
            "execution request is not awaiting approval"
        );
        if env::block_timestamp() >= record.expires_at_ns {
            record.status = ExecutionStatus::Expired;
            self.execution_requests.insert(request_id.clone(), record);
            self.emit_event(
                "execution_expired",
                near_sdk::serde_json::json!({ "request_id": request_id }),
            );
            return;
        }
        assert!(!self.paused, "protocol is paused");
        assert!(
            registry::is_valid_hash(&expected_action_hash)
                && expected_action_hash == record.action_hash,
            "action hash does not match proposed request"
        );
        let reference = ChainReference {
            namespace: record.request.chain_namespace.clone(),
            chain_id: record.request.chain_id.clone(),
        };
        let project = self
            .projects
            .get(&record.request.project_id)
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
        let policy = self
            .get_active_execution_policy(
                record.request.project_id.clone(),
                record.request.chain_namespace.clone(),
                record.request.chain_id.clone(),
            )
            .unwrap_or_else(|| env::panic_str("execution policy not configured"));
        assert_eq!(
            policy.version, record.request.policy_version,
            "request policy version is no longer active"
        );

        let approver = env::predecessor_account_id();
        assert_ne!(
            approver, record.proposer,
            "proposer cannot approve own request"
        );
        assert!(
            policy.approvers.contains(&approver),
            "caller is not an approver"
        );
        let approval_key = registry::execution_approval_key(&request_id, &approver);
        assert!(
            self.execution_approvals.get(&approval_key).is_none(),
            "approver already voted"
        );
        self.execution_approvals.insert(approval_key, true);
        record.approval_count = record
            .approval_count
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str("approval count overflow"));
        if record.approval_count >= policy.approval_threshold {
            record.status = ExecutionStatus::Approved;
        }
        self.execution_requests
            .insert(request_id.clone(), record.clone());
        self.emit_event(
            "execution_approved",
            near_sdk::serde_json::json!({
                "request_id": request_id,
                "approver": approver,
                "action_hash": record.action_hash,
                "approval_count": record.approval_count,
                "status": record.status
            }),
        );
    }

    #[payable]
    pub fn request_signature(
        &mut self,
        request_id: String,
        expected_action_hash: String,
    ) -> Promise {
        assert_eq!(
            env::attached_deposit(),
            NearToken::from_near(1),
            "attach exactly 1 NEAR for the testnet signature request"
        );
        assert!(!self.paused, "protocol is paused");
        let mut record = self
            .execution_requests
            .get(&request_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("execution request not found"));
        assert_eq!(
            record.status,
            ExecutionStatus::Approved,
            "execution request is not approved"
        );
        assert!(
            matches!(
                record.signature_status,
                SignatureStatus::NotRequested | SignatureStatus::Failed
            ),
            "signature already requested or issued"
        );
        assert!(
            env::block_timestamp() < record.expires_at_ns,
            "execution request has expired"
        );
        assert!(
            registry::is_valid_hash(&expected_action_hash)
                && expected_action_hash == record.action_hash,
            "action hash does not match approved request"
        );
        let serialized = near_sdk::serde_json::to_vec(&record.request)
            .unwrap_or_else(|_| env::panic_str("failed to encode execution request"));
        assert_eq!(
            registry::hash_to_hex(&env::sha256(&serialized)),
            record.action_hash,
            "stored request no longer matches action hash"
        );

        let reference = ChainReference {
            namespace: record.request.chain_namespace.clone(),
            chain_id: record.request.chain_id.clone(),
        };
        assert_eq!(reference.namespace, "eip155", "only EVM signing is enabled");
        let project = self
            .projects
            .get(&record.request.project_id)
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
            chain.capabilities.chain_signatures,
            "chain does not enable Chain Signatures"
        );
        assert_eq!(chain.family, "evm", "destination chain must be EVM");
        assert_eq!(
            chain.signing_scheme, "secp256k1",
            "EVM signing requires secp256k1"
        );
        assert!(
            record.request.gas_limit.is_some(),
            "EVM gas limit is required"
        );
        let signing_payload_hash = record
            .request
            .signing_payload_hash
            .as_deref()
            .unwrap_or_else(|| env::panic_str("EVM signing payload hash is required"));
        assert!(
            registry::is_valid_secp256k1_digest(signing_payload_hash),
            "signing payload hash is not a valid secp256k1 scalar"
        );

        let policy = self
            .get_active_execution_policy(
                record.request.project_id.clone(),
                reference.namespace.clone(),
                reference.chain_id.clone(),
            )
            .unwrap_or_else(|| env::panic_str("execution policy not configured"));
        assert_eq!(
            policy.version, record.request.policy_version,
            "request policy version is no longer active"
        );
        assert_eq!(
            policy.policy_hash, record.policy_hash,
            "request policy hash is no longer active"
        );
        let caller = env::predecessor_account_id();
        assert!(
            caller == self.administrator || policy.operators.contains(&caller),
            "operator or administrator only"
        );

        let binding = self
            .get_active_signer(
                record.request.project_id.clone(),
                reference.namespace.clone(),
                reference.chain_id.clone(),
                "deployer".to_owned(),
            )
            .unwrap_or_else(|| env::panic_str("active EVM signer binding not configured"));
        assert_eq!(
            binding.signing_scheme, "secp256k1",
            "active EVM signer must use secp256k1"
        );
        let approved_key_version = record
            .request
            .signer_key_version
            .unwrap_or_else(|| env::panic_str("approved request is missing signer key version"));
        assert_eq!(
            binding.key_version, approved_key_version,
            "active signer binding version differs from approved request"
        );
        let chain_signatures = self
            .chain_signatures
            .clone()
            .unwrap_or_else(|| env::panic_str("testnet Chain Signatures contract not configured"));

        record.signature_status = SignatureStatus::Pending;
        record.signature_public_key = Some(chain_signatures.public_key.clone());
        record.signature_response = None;
        self.execution_requests
            .insert(request_id.clone(), record.clone());
        self.emit_event(
            "signature_requested",
            near_sdk::serde_json::json!({
                "request_id": request_id,
                "action_hash": record.action_hash,
                "signing_payload_hash": signing_payload_hash,
                "contract_id": chain_signatures.contract_id,
                "public_key": chain_signatures.public_key,
                "key_version": binding.key_version
            }),
        );

        let payload: Vec<u8> = (0..32)
            .map(|index| {
                u8::from_str_radix(&signing_payload_hash[2 + index * 2..4 + index * 2], 16)
                    .expect("validated signing payload hash contains hex bytes")
            })
            .collect();
        let args = near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({
            "request": {
                "path": binding.derivation_path,
                "payload": payload,
                "key_version": binding.key_version
            }
        }))
        .unwrap_or_else(|_| env::panic_str("failed to encode Chain Signatures request"));
        let callback_args = near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({
            "request_id": request_id
        }))
        .unwrap_or_else(|_| env::panic_str("failed to encode signature callback"));
        Promise::new(chain_signatures.contract_id)
            .function_call(
                "sign".to_owned(),
                args,
                NearToken::from_near(1),
                Gas::from_tgas(250),
            )
            .then(Promise::new(env::current_account_id()).function_call(
                "on_signature_result".to_owned(),
                callback_args,
                NearToken::from_yoctonear(0),
                Gas::from_tgas(20),
            ))
    }

    #[private]
    pub fn on_signature_result(&mut self, request_id: String) -> Option<String> {
        let mut record = self
            .execution_requests
            .get(&request_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("execution request not found"));
        if record.signature_status != SignatureStatus::Pending
            || record.status != ExecutionStatus::Approved
        {
            if record.signature_status == SignatureStatus::Pending {
                record.signature_status = SignatureStatus::Failed;
                record.signature_response = None;
                self.execution_requests.insert(request_id.clone(), record);
            }
            return None;
        }
        if env::block_timestamp() >= record.expires_at_ns {
            record.status = ExecutionStatus::Expired;
            record.signature_status = SignatureStatus::Failed;
            record.signature_response = None;
            self.execution_requests
                .insert(request_id.clone(), record.clone());
            self.emit_event(
                "execution_expired",
                near_sdk::serde_json::json!({ "request_id": request_id }),
            );
            return None;
        }
        let active_policy = self.get_active_execution_policy(
            record.request.project_id.clone(),
            record.request.chain_namespace.clone(),
            record.request.chain_id.clone(),
        );
        if active_policy.is_none_or(|policy| policy.version != record.request.policy_version) {
            record.signature_status = SignatureStatus::Failed;
            record.signature_response = None;
            self.execution_requests.insert(request_id.clone(), record);
            self.emit_event(
                "signature_failed",
                near_sdk::serde_json::json!({
                    "request_id": request_id,
                    "reason": "execution policy changed while signature was pending"
                }),
            );
            return None;
        }
        if self.paused {
            record.signature_status = SignatureStatus::Failed;
            record.signature_response = None;
            self.execution_requests.insert(request_id.clone(), record);
            self.emit_event(
                "signature_failed",
                near_sdk::serde_json::json!({
                    "request_id": request_id,
                    "reason": "protocol paused while signature was pending"
                }),
            );
            return None;
        }
        match env::promise_result_checked(0, 4096) {
            Ok(bytes) => {
                let response =
                    near_sdk::serde_json::from_slice::<near_sdk::serde_json::Value>(&bytes).ok();
                let is_secp256k1 = response.as_ref().is_some_and(|value| {
                    let is_hex = |candidate: Option<&str>| {
                        candidate.is_some_and(|hex| {
                            !hex.is_empty()
                                && hex.len() % 2 == 0
                                && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
                        })
                    };
                    let has_expected_fields = value.as_object().is_some_and(|object| {
                        object.len() == 3
                            && object.contains_key("big_r")
                            && object.contains_key("s")
                            && object.contains_key("recovery_id")
                    });
                    has_expected_fields
                        && is_hex(
                            value
                                .get("big_r")
                                .and_then(|point| point.get("affine_point"))
                                .and_then(|point| point.as_str()),
                        )
                        && is_hex(
                            value
                                .get("s")
                                .and_then(|scalar| scalar.get("scalar"))
                                .and_then(|scalar| scalar.as_str()),
                        )
                        && value
                            .get("recovery_id")
                            .and_then(|id| id.as_u64())
                            .is_some_and(|id| id <= u8::MAX as u64)
                });
                if is_secp256k1 {
                    let signature_json = near_sdk::serde_json::to_string(
                        response.as_ref().expect("validated signature response"),
                    )
                    .unwrap_or_else(|_| env::panic_str("failed to encode signature response"));
                    record.signature_status = SignatureStatus::Issued;
                    record.signature_response = Some(signature_json.clone());
                    self.execution_requests
                        .insert(request_id.clone(), record.clone());
                    self.emit_event(
                        "signature_issued",
                        near_sdk::serde_json::json!({
                            "request_id": request_id,
                            "signing_payload_hash": record.request.signing_payload_hash
                        }),
                    );
                    Some(signature_json)
                } else {
                    record.signature_status = SignatureStatus::Failed;
                    record.signature_response = None;
                    self.execution_requests.insert(request_id.clone(), record);
                    self.emit_event(
                        "signature_failed",
                        near_sdk::serde_json::json!({
                            "request_id": request_id,
                            "reason": "invalid secp256k1 response"
                        }),
                    );
                    None
                }
            }
            _ => {
                record.signature_status = SignatureStatus::Failed;
                record.signature_response = None;
                self.execution_requests.insert(request_id.clone(), record);
                self.emit_event(
                    "signature_failed",
                    near_sdk::serde_json::json!({
                        "request_id": request_id,
                        "reason": "Chain Signatures call failed"
                    }),
                );
                None
            }
        }
    }

    pub fn cancel_execution(&mut self, request_id: String) {
        let mut record = self
            .execution_requests
            .get(&request_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("execution request not found"));
        let caller = env::predecessor_account_id();
        assert!(
            caller == record.proposer || caller == self.administrator,
            "proposer or administrator only"
        );
        if env::block_timestamp() >= record.expires_at_ns {
            record.status = ExecutionStatus::Expired;
            if record.signature_status == SignatureStatus::Pending {
                record.signature_status = SignatureStatus::Failed;
                record.signature_response = None;
            }
            self.execution_requests.insert(request_id.clone(), record);
            self.emit_event(
                "execution_expired",
                near_sdk::serde_json::json!({ "request_id": request_id }),
            );
            return;
        }
        assert!(
            matches!(
                record.status,
                ExecutionStatus::AwaitingApproval | ExecutionStatus::Approved
            ),
            "execution request cannot be cancelled"
        );
        record.status = ExecutionStatus::Cancelled;
        if record.signature_status == SignatureStatus::Pending {
            record.signature_status = SignatureStatus::Failed;
            record.signature_response = None;
        }
        self.execution_requests.insert(request_id.clone(), record);
        self.emit_event(
            "execution_cancelled",
            near_sdk::serde_json::json!({ "request_id": request_id, "caller": caller }),
        );
    }

    pub fn expire_execution(&mut self, request_id: String) {
        let mut record = self
            .execution_requests
            .get(&request_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("execution request not found"));
        assert!(
            matches!(
                record.status,
                ExecutionStatus::AwaitingApproval | ExecutionStatus::Approved
            ),
            "execution request cannot be expired"
        );
        assert!(
            env::block_timestamp() >= record.expires_at_ns,
            "execution request has not expired"
        );
        record.status = ExecutionStatus::Expired;
        if record.signature_status == SignatureStatus::Pending {
            record.signature_status = SignatureStatus::Failed;
            record.signature_response = None;
        }
        self.execution_requests.insert(request_id.clone(), record);
        self.emit_event(
            "execution_expired",
            near_sdk::serde_json::json!({ "request_id": request_id }),
        );
    }

    pub fn get_execution_request(&self, request_id: String) -> Option<ExecutionRequestRecord> {
        self.execution_requests.get(&request_id).cloned()
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
    #[allow(deprecated)]
    use near_sdk::test_utils::testing_env_with_promise_results;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, AccountId, NearToken};
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

    fn set_predecessor_at(value: &str, timestamp_ns: u64) {
        testing_env!(VMContextBuilder::new()
            .predecessor_account_id(account(value))
            .block_timestamp(timestamp_ns)
            .build());
    }

    fn set_predecessor_with_near_deposit(value: &str, amount_near: u128) {
        testing_env!(VMContextBuilder::new()
            .predecessor_account_id(account(value))
            .attached_deposit(NearToken::from_near(amount_near))
            .build());
    }

    fn set_predecessor_with_near_deposit_at(value: &str, amount_near: u128, timestamp_ns: u64) {
        testing_env!(VMContextBuilder::new()
            .predecessor_account_id(account(value))
            .attached_deposit(NearToken::from_near(amount_near))
            .block_timestamp(timestamp_ns)
            .build());
    }

    #[allow(deprecated)]
    fn set_callback_promise_result(result: near_sdk::PromiseResult) {
        testing_env_with_promise_results(VMContextBuilder::new().build(), result);
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
            vec![account("operator.near")],
            vec![account("alice.near"), account("bob.near")],
            2,
        );
    }

    fn execution_request(
        request_id: &str,
        idempotency_key: &str,
        expires_at: &str,
    ) -> ExecutionRequest {
        ExecutionRequest {
            protocol_version: "1".to_owned(),
            request_id: request_id.to_owned(),
            project_id: PROJECT_ID.to_owned(),
            chain_namespace: "eip155".to_owned(),
            chain_id: "84532".to_owned(),
            operation: "deploy_project_component".to_owned(),
            target: "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
            value_base_units: "500".to_owned(),
            gas_limit: Some("120000".to_owned()),
            calldata_hash: format!("0x{}", "a".repeat(64)),
            signing_payload_hash: Some(format!("0x{}", "b".repeat(64))),
            signer_key_version: Some(0),
            expected_effect_hash: Some(format!("0x{}", "c".repeat(64))),
            nonce: "0".to_owned(),
            gas_policy: GasPolicy {
                max_fee: "10".to_owned(),
                max_priority_fee: "2".to_owned(),
            },
            expires_at: expires_at.to_owned(),
            policy_version: "policy-v1".to_owned(),
            idempotency_key: idempotency_key.to_owned(),
        }
    }

    fn configure_signing_fixture(hub: &mut KokonutProtocolHub, request_id: &str) -> String {
        register_evm_signing_project(hub);
        configure_execution_policy(hub, "policy-v1");
        hub.configure_signer(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            "deployer".to_owned(),
            format!("kokonut/v1/project/{PROJECT_ID}/eip155/84532/deployer"),
            "secp256k1".to_owned(),
            "0x1234567890abcdef1234567890abcdef12345678".to_owned(),
            0,
        );
        hub.activate_signer(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            "deployer".to_owned(),
            0,
        );
        hub.configure_chain_signatures(
            account("v1.signer-prod.testnet"),
            "secp256k1:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        );
        assert_eq!(
            hub.get_chain_signatures_config(),
            Some(ChainSignaturesConfig {
                contract_id: account("v1.signer-prod.testnet"),
                public_key:
                    "secp256k1:1111111111111111111111111111111111111111111111111111111111111111"
                        .to_owned(),
            })
        );

        set_predecessor("operator.near");
        hub.propose_execution(execution_request(
            request_id,
            "idem-signature-test",
            "2035-01-01T00:00:00Z",
        ));
        let action_hash = hub
            .get_execution_request(request_id.to_owned())
            .expect("proposed request exists")
            .action_hash;
        set_predecessor("alice.near");
        hub.approve_execution(request_id.to_owned(), action_hash.clone());
        set_predecessor("bob.near");
        hub.approve_execution(request_id.to_owned(), action_hash.clone());
        action_hash
    }

    fn register_evm_signing_project(hub: &mut KokonutProtocolHub) {
        hub.register_chain(
            "eip155".to_owned(),
            "84532".to_owned(),
            "evm".to_owned(),
            "secp256k1".to_owned(),
            ChainCapabilities {
                chain_signatures: true,
                contract_deploy: true,
                eas: false,
                safe: true,
                baal: false,
                intents_settlement: false,
            },
            true,
        );
        hub.register_project(
            PROJECT_ID.to_owned(),
            PROFILE_HASH.to_owned(),
            vec![ChainReference {
                namespace: "eip155".to_owned(),
                chain_id: "84532".to_owned(),
            }],
        );
    }

    #[test]
    fn chain_signatures_configuration_rejects_mainnet_contracts() {
        let mut hub = initialized_hub();
        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.configure_chain_signatures(
                account("v1.signer"),
                "secp256k1:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_owned(),
            )
        }));
        assert!(result.is_err());
    }

    #[test]
    fn administrator_can_update_testnet_signer_contract_and_public_key() {
        let mut hub = initialized_hub();
        hub.configure_chain_signatures(
            account("v1.signer-prod.testnet"),
            "secp256k1:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        );
        hub.configure_chain_signatures(
            account("v2.signer.testnet"),
            "secp256k1:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        );

        assert_eq!(
            hub.get_chain_signatures_config(),
            Some(ChainSignaturesConfig {
                contract_id: account("v2.signer.testnet"),
                public_key:
                    "secp256k1:2222222222222222222222222222222222222222222222222222222222222222"
                        .to_owned(),
            })
        );
    }

    #[test]
    fn chain_signatures_configuration_rejects_non_secp256k1_public_keys() {
        let mut hub = initialized_hub();
        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.configure_chain_signatures(
                account("v1.signer-prod.testnet"),
                "ed25519:11111111111111111111111111111111111111111111111111111111".to_owned(),
            )
        }));
        assert!(result.is_err());
        assert!(hub.get_chain_signatures_config().is_none());
    }

    #[test]
    fn non_administrator_cannot_configure_chain_signatures() {
        let mut hub = initialized_hub();
        set_predecessor(STRANGER);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.configure_chain_signatures(
                account("v1.signer-prod.testnet"),
                "secp256k1:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_owned(),
            )
        }));

        assert!(result.is_err());
        assert!(hub.get_chain_signatures_config().is_none());
    }

    #[test]
    fn approved_execution_can_request_testnet_signature_once() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c5a";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);

        hub.request_signature(request_id.to_owned(), action_hash.clone())
            .as_return()
            .detach();

        let receipts = near_sdk::test_utils::get_created_receipts();
        let signer_receipt = receipts
            .iter()
            .find(|receipt| receipt.receiver_id == account("v1.signer-prod.testnet"))
            .expect("signer cross-contract receipt created");
        let sign_args = signer_receipt
            .actions
            .iter()
            .find_map(|action| match action {
                near_sdk::mock::MockAction::FunctionCallWeight {
                    method_name,
                    args,
                    attached_deposit,
                    prepaid_gas,
                    ..
                } if method_name == b"sign" => Some((args, attached_deposit, prepaid_gas)),
                _ => None,
            })
            .expect("sign function call created");
        assert_eq!(
            near_sdk::serde_json::from_slice::<near_sdk::serde_json::Value>(sign_args.0).unwrap(),
            near_sdk::serde_json::json!({
                "request": {
                    "path": format!("kokonut/v1/project/{PROJECT_ID}/eip155/84532/deployer"),
                    "payload": vec![0xbb; 32],
                    "key_version": 0
                }
            })
        );
        assert_eq!(*sign_args.1, NearToken::from_near(1));
        assert_eq!(*sign_args.2, Gas::from_tgas(250));

        let record = hub
            .get_execution_request(request_id.to_owned())
            .expect("request exists");
        assert_eq!(record.status, ExecutionStatus::Approved);
        assert_eq!(record.signature_status, SignatureStatus::Pending);
        assert_eq!(
            record.signature_public_key.as_deref(),
            Some("secp256k1:1111111111111111111111111111111111111111111111111111111111111111")
        );
        assert_eq!(record.action_hash, action_hash);
        assert!(record.signature_response.is_none());
    }

    #[test]
    fn pending_signature_request_cannot_be_submitted_again() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c5d";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash.clone())
            .as_return()
            .detach();

        set_predecessor_with_near_deposit("operator.near", 1);
        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.request_signature(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        assert_eq!(
            hub.get_execution_request(request_id.to_owned())
                .unwrap()
                .signature_status,
            SignatureStatus::Pending
        );
    }

    #[test]
    fn signature_request_rejects_signer_rotation_after_approval() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c64";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor(ADMIN);
        hub.configure_signer(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            "deployer".to_owned(),
            format!("kokonut/v1/project/{PROJECT_ID}/eip155/84532/deployer"),
            "secp256k1".to_owned(),
            "0x2234567890abcdef1234567890abcdef12345678".to_owned(),
            2,
        );
        hub.activate_signer(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            "deployer".to_owned(),
            2,
        );
        set_predecessor_with_near_deposit("operator.near", 1);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.request_signature(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();
        assert_eq!(record.status, ExecutionStatus::Approved);
        assert_eq!(record.signature_status, SignatureStatus::NotRequested);
    }

    #[test]
    fn signature_request_rechecks_the_active_policy_version() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c5f";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor(ADMIN);
        configure_execution_policy(&mut hub, "policy-v2");
        set_predecessor_with_near_deposit("operator.near", 1);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.request_signature(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();
        assert_eq!(record.status, ExecutionStatus::Approved);
        assert_eq!(record.signature_status, SignatureStatus::NotRequested);
    }

    #[test]
    fn pause_blocks_signature_requests_after_approval() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c5e";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor(ADMIN);
        hub.pause();
        set_predecessor_with_near_deposit("operator.near", 1);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.request_signature(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();
        assert_eq!(record.status, ExecutionStatus::Approved);
        assert_eq!(record.signature_status, SignatureStatus::NotRequested);
    }

    #[test]
    fn signature_request_requires_exactly_one_near_deposit() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c63";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 0);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.request_signature(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        assert_eq!(
            hub.get_execution_request(request_id.to_owned())
                .unwrap()
                .signature_status,
            SignatureStatus::NotRequested
        );
    }

    #[test]
    fn signature_request_rejects_expired_approved_execution() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c61";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit_at("operator.near", 1, u64::MAX);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.request_signature(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();
        assert_eq!(record.signature_status, SignatureStatus::NotRequested);
    }

    #[test]
    fn non_operator_cannot_request_signature() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c62";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit(STRANGER, 1);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.request_signature(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();
        assert_eq!(record.signature_status, SignatureStatus::NotRequested);
    }

    #[test]
    fn signature_callback_persists_secp256k1_result() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c5b";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash.clone())
            .as_return()
            .detach();
        set_predecessor(ADMIN);
        hub.configure_chain_signatures(
            account("v2.signer.testnet"),
            "secp256k1:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        );
        set_callback_promise_result(near_sdk::PromiseResult::Successful(
            br#"{"big_r":{"affine_point":"04"},"s":{"scalar":"02"},"recovery_id":0}"#.to_vec(),
        ));

        hub.on_signature_result(request_id.to_owned());

        let record = hub
            .get_execution_request(request_id.to_owned())
            .expect("request exists");
        assert_eq!(record.signature_status, SignatureStatus::Issued);
        assert_eq!(
            record.signature_public_key.as_deref(),
            Some("secp256k1:1111111111111111111111111111111111111111111111111111111111111111")
        );
        assert!(record
            .signature_response
            .as_deref()
            .unwrap()
            .contains("big_r"));

        set_predecessor_with_near_deposit("operator.near", 1);
        let retry = catch_unwind(AssertUnwindSafe(|| {
            hub.request_signature(request_id.to_owned(), action_hash)
        }));
        assert!(retry.is_err());
    }

    #[test]
    fn cancellation_invalidates_a_pending_signature_callback() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c71";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash)
            .as_return()
            .detach();

        set_predecessor("operator.near");
        hub.cancel_execution(request_id.to_owned());
        set_callback_promise_result(near_sdk::PromiseResult::Successful(
            br#"{"big_r":{"affine_point":"04"},"s":{"scalar":"02"},"recovery_id":0}"#.to_vec(),
        ));

        let callback_result = hub.on_signature_result(request_id.to_owned());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();

        assert!(callback_result.is_none());
        assert_eq!(record.status, ExecutionStatus::Cancelled);
        assert_eq!(record.signature_status, SignatureStatus::Failed);
        assert!(record.signature_response.is_none());
    }

    #[test]
    fn expiry_invalidates_a_pending_signature_callback() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c72";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash)
            .as_return()
            .detach();

        set_predecessor_at("stranger.near", u64::MAX);
        hub.expire_execution(request_id.to_owned());
        set_callback_promise_result(near_sdk::PromiseResult::Successful(
            br#"{"big_r":{"affine_point":"04"},"s":{"scalar":"02"},"recovery_id":0}"#.to_vec(),
        ));

        let callback_result = hub.on_signature_result(request_id.to_owned());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();

        assert!(callback_result.is_none());
        assert_eq!(record.status, ExecutionStatus::Expired);
        assert_eq!(record.signature_status, SignatureStatus::Failed);
        assert!(record.signature_response.is_none());
    }

    #[test]
    fn signature_callback_rejects_policy_version_changed_while_pending() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c73";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash)
            .as_return()
            .detach();

        set_predecessor(ADMIN);
        configure_execution_policy(&mut hub, "policy-v2");
        set_callback_promise_result(near_sdk::PromiseResult::Successful(
            br#"{"big_r":{"affine_point":"04"},"s":{"scalar":"02"},"recovery_id":0}"#.to_vec(),
        ));

        let callback_result = hub.on_signature_result(request_id.to_owned());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();

        assert!(callback_result.is_none());
        assert_eq!(record.status, ExecutionStatus::Approved);
        assert_eq!(record.signature_status, SignatureStatus::Failed);
        assert!(record.signature_response.is_none());
    }

    #[test]
    fn signature_callback_expires_request_at_deadline_without_expire_call() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c74";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash)
            .as_return()
            .detach();
        let expires_at_ns = hub
            .get_execution_request(request_id.to_owned())
            .expect("request exists")
            .expires_at_ns;
        #[allow(deprecated)]
        testing_env_with_promise_results(
            VMContextBuilder::new()
                .block_timestamp(expires_at_ns)
                .build(),
            near_sdk::PromiseResult::Successful(
                br#"{"big_r":{"affine_point":"04"},"s":{"scalar":"02"},"recovery_id":0}"#.to_vec(),
            ),
        );

        let callback_result = hub.on_signature_result(request_id.to_owned());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();

        assert!(callback_result.is_none());
        assert_eq!(record.status, ExecutionStatus::Expired);
        assert_eq!(record.signature_status, SignatureStatus::Failed);
        assert!(record.signature_response.is_none());
    }

    #[test]
    fn signature_callback_is_suppressed_when_protocol_pauses_while_pending() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c75";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash)
            .as_return()
            .detach();

        set_predecessor(ADMIN);
        hub.pause();
        set_callback_promise_result(near_sdk::PromiseResult::Successful(
            br#"{"big_r":{"affine_point":"04"},"s":{"scalar":"02"},"recovery_id":0}"#.to_vec(),
        ));

        let callback_result = hub.on_signature_result(request_id.to_owned());
        let record = hub.get_execution_request(request_id.to_owned()).unwrap();

        assert!(callback_result.is_none());
        assert_eq!(record.status, ExecutionStatus::Approved);
        assert_eq!(record.signature_status, SignatureStatus::Failed);
        assert!(record.signature_response.is_none());
    }

    #[test]
    fn signature_callback_rejects_unexpected_response_shape() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c60";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash)
            .as_return()
            .detach();
        set_callback_promise_result(near_sdk::PromiseResult::Successful(
            br#"{"big_r":{"affine_point":"04"},"s":{"scalar":"02"},"recovery_id":0,"extra":true}"#
                .to_vec(),
        ));

        let response = hub.on_signature_result(request_id.to_owned());

        let record = hub.get_execution_request(request_id.to_owned()).unwrap();
        assert_eq!(record.signature_status, SignatureStatus::Failed);
        assert!(record.signature_response.is_none());
        assert!(response.is_none());
    }

    #[test]
    fn signature_callback_marks_failed_result_retryable() {
        let mut hub = initialized_hub();
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c5c";
        let action_hash = configure_signing_fixture(&mut hub, request_id);
        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash.clone())
            .as_return()
            .detach();
        set_callback_promise_result(near_sdk::PromiseResult::Failed);

        hub.on_signature_result(request_id.to_owned());

        let record = hub
            .get_execution_request(request_id.to_owned())
            .expect("request exists");
        assert_eq!(record.signature_status, SignatureStatus::Failed);
        assert!(record.signature_response.is_none());

        set_predecessor_with_near_deposit("operator.near", 1);
        hub.request_signature(request_id.to_owned(), action_hash)
            .as_return()
            .detach();
        assert_eq!(
            hub.get_execution_request(request_id.to_owned())
                .unwrap()
                .signature_status,
            SignatureStatus::Pending
        );
    }

    #[test]
    fn authorized_operator_can_propose_policy_compliant_execution() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");

        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4c";
        hub.propose_execution(execution_request(
            request_id,
            "idem-deploy-1",
            "2035-01-01T00:00:00Z",
        ));
        let record = hub
            .get_execution_request(request_id.to_owned())
            .expect("proposed execution is stored");

        assert_eq!(record.status, ExecutionStatus::AwaitingApproval);
        assert_eq!(record.proposer, account("operator.near"));
        assert_eq!(record.approval_count, 0);
        assert!(registry::is_valid_hash(&record.action_hash));
    }

    #[test]
    fn pause_blocks_approvals_for_pending_requests() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e";
        hub.propose_execution(execution_request(
            request_id,
            "idem-paused-approval",
            "2035-01-01T00:00:00Z",
        ));
        let action_hash = hub
            .get_execution_request(request_id.to_owned())
            .expect("request exists")
            .action_hash;
        set_predecessor(ADMIN);
        hub.pause();
        set_predecessor("alice.near");

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.approve_execution(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        let pending = hub
            .get_execution_request(request_id.to_owned())
            .expect("request remains pending");
        assert_eq!(pending.approval_count, 0);
        assert_eq!(pending.status, ExecutionStatus::AwaitingApproval);
    }

    #[test]
    fn approval_is_rejected_after_active_policy_version_changes() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e";
        hub.propose_execution(execution_request(
            request_id,
            "idem-old-policy",
            "2035-01-01T00:00:00Z",
        ));
        let action_hash = hub
            .get_execution_request(request_id.to_owned())
            .expect("request exists")
            .action_hash;
        set_predecessor(ADMIN);
        configure_execution_policy(&mut hub, "policy-v2");
        set_predecessor("alice.near");

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.approve_execution(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        let unchanged = hub
            .get_execution_request(request_id.to_owned())
            .expect("request remains auditable");
        assert_eq!(unchanged.status, ExecutionStatus::AwaitingApproval);
        assert_eq!(unchanged.approval_count, 0);
    }

    #[test]
    fn execution_proposer_cannot_approve_own_request() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        hub.configure_execution_policy(
            PROJECT_ID.to_owned(),
            "eip155".to_owned(),
            "84532".to_owned(),
            "policy-v1".to_owned(),
            vec!["deploy_project_component".to_owned()],
            vec!["0x1234567890abcdef1234567890abcdef12345678".to_owned()],
            "1000000".to_owned(),
            vec![account("alice.near")],
            vec![account("alice.near"), account("bob.near")],
            1,
        );
        set_predecessor("alice.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e";
        hub.propose_execution(execution_request(
            request_id,
            "idem-self-approval",
            "2035-01-01T00:00:00Z",
        ));
        let action_hash = hub
            .get_execution_request(request_id.to_owned())
            .expect("request exists")
            .action_hash;

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.approve_execution(request_id.to_owned(), action_hash)
        }));

        assert!(result.is_err());
        let pending = hub
            .get_execution_request(request_id.to_owned())
            .expect("request remains pending");
        assert_eq!(pending.approval_count, 0);
        assert_eq!(pending.status, ExecutionStatus::AwaitingApproval);
    }

    #[test]
    fn approval_binds_the_action_hash_and_requires_distinct_quorum() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4d";
        hub.propose_execution(execution_request(
            request_id,
            "idem-deploy-2",
            "2035-01-01T00:00:00Z",
        ));
        let action_hash = hub
            .get_execution_request(request_id.to_owned())
            .expect("execution request exists")
            .action_hash;

        set_predecessor("alice.near");
        let wrong_hash = catch_unwind(AssertUnwindSafe(|| {
            hub.approve_execution(
                request_id.to_owned(),
                "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
            )
        }));
        assert!(wrong_hash.is_err());
        hub.approve_execution(request_id.to_owned(), action_hash.clone());
        let after_first = hub
            .get_execution_request(request_id.to_owned())
            .expect("execution request remains stored");
        let duplicate_vote = catch_unwind(AssertUnwindSafe(|| {
            hub.approve_execution(request_id.to_owned(), action_hash.clone())
        }));

        assert!(duplicate_vote.is_err());
        assert_eq!(after_first.approval_count, 1);
        assert_eq!(after_first.status, ExecutionStatus::AwaitingApproval);

        set_predecessor("bob.near");
        hub.approve_execution(request_id.to_owned(), action_hash);
        let approved = hub
            .get_execution_request(request_id.to_owned())
            .expect("execution request remains stored");
        assert_eq!(approved.approval_count, 2);
        assert_eq!(approved.status, ExecutionStatus::Approved);
    }

    #[test]
    fn duplicate_execution_request_id_is_rejected() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4d";
        let request = execution_request(request_id, "idem-deploy-replay", "2035-01-01T00:00:00Z");
        hub.propose_execution(request.clone());

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn duplicate_execution_idempotency_key_is_rejected() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        hub.propose_execution(execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4d",
            "idem-deploy-replay",
            "2035-01-01T00:00:00Z",
        ));

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.propose_execution(execution_request(
                "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
                "idem-deploy-replay",
                "2035-01-01T00:00:00Z",
            ))
        }));

        assert!(result.is_err());
    }

    #[test]
    fn inactive_project_cannot_propose_execution() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        hub.set_project_status(PROJECT_ID.to_owned(), false);
        set_predecessor("operator.near");

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.propose_execution(execution_request(
                "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
                "idem-inactive-project",
                "2035-01-01T00:00:00Z",
            ))
        }));

        assert!(result.is_err());
    }

    #[test]
    fn disabled_chain_cannot_accept_execution_proposal() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        hub.set_chain_enabled("eip155".to_owned(), "84532".to_owned(), false);
        set_predecessor("operator.near");

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.propose_execution(execution_request(
                "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
                "idem-disabled-chain",
                "2035-01-01T00:00:00Z",
            ))
        }));

        assert!(result.is_err());
    }

    #[test]
    fn non_operator_cannot_propose_execution() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor(STRANGER);

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.propose_execution(execution_request(
                "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
                "idem-unauthorized",
                "2035-01-01T00:00:00Z",
            ))
        }));

        assert!(result.is_err());
    }

    #[test]
    fn execution_proposal_rejects_priority_fee_above_max_fee() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let mut request = execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
            "idem-invalid-fees",
            "2035-01-01T00:00:00Z",
        );
        request.gas_policy.max_priority_fee = "11".to_owned();

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn execution_proposal_rejects_missing_evm_signing_fields() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let mut request = execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
            "idem-missing-signing-fields",
            "2035-01-01T00:00:00Z",
        );
        request.signing_payload_hash = None;

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn execution_proposal_rejects_missing_signer_key_version() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let mut request = execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c65",
            "idem-missing-signer-version",
            "2035-01-01T00:00:00Z",
        );
        request.signer_key_version = None;

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn execution_proposal_rejects_missing_expected_effect_hash() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let mut request = execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c67",
            "idem-missing-effect-hash",
            "2035-01-01T00:00:00Z",
        );
        request.expected_effect_hash = None;

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn execution_proposal_accepts_zero_signer_key_version() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c66";
        let mut request = execution_request(
            request_id,
            "idem-zero-signer-version",
            "2035-01-01T00:00:00Z",
        );
        request.signer_key_version = Some(0);

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_ok());
        assert_eq!(
            hub.get_execution_request(request_id.to_owned())
                .unwrap()
                .request
                .signer_key_version,
            Some(0)
        );
    }

    #[test]
    fn execution_proposal_rejects_zero_gas_limit() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let mut request = execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
            "idem-zero-gas-limit",
            "2035-01-01T00:00:00Z",
        );
        request.gas_limit = Some("0".to_owned());

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn execution_proposal_rejects_malformed_signing_payload_hash() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let mut request = execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4d",
            "idem-malformed-signing-hash",
            "2035-01-01T00:00:00Z",
        );
        request.signing_payload_hash = Some("0x1234".to_owned());

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn execution_proposal_rejects_value_above_policy_cap() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let mut request = execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
            "idem-over-cap",
            "2035-01-01T00:00:00Z",
        );
        request.value_base_units = "1000001".to_owned();

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn execution_proposal_rejects_target_outside_policy_allowlist() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let mut request = execution_request(
            "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4f",
            "idem-wrong-target",
            "2035-01-01T00:00:00Z",
        );
        request.target = "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd".to_owned();

        let result = catch_unwind(AssertUnwindSafe(|| hub.propose_execution(request)));

        assert!(result.is_err());
    }

    #[test]
    fn paused_protocol_rejects_execution_proposals() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        hub.pause();
        set_predecessor("operator.near");

        let result = catch_unwind(AssertUnwindSafe(|| {
            hub.propose_execution(execution_request(
                "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e",
                "idem-paused",
                "2035-01-01T00:00:00Z",
            ))
        }));

        assert!(result.is_err());
    }

    #[test]
    fn expired_request_transitions_to_expired_instead_of_accepting_approval() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e";
        hub.propose_execution(execution_request(
            request_id,
            "idem-expired",
            "1970-01-01T00:00:00.000000001Z",
        ));
        let action_hash = hub
            .get_execution_request(request_id.to_owned())
            .expect("request exists before expiry")
            .action_hash;
        set_predecessor_at("alice.near", 1);
        hub.approve_execution(request_id.to_owned(), action_hash);

        let expired = hub
            .get_execution_request(request_id.to_owned())
            .expect("expired request remains auditable");
        assert_eq!(expired.status, ExecutionStatus::Expired);
        assert_eq!(expired.approval_count, 0);
    }

    #[test]
    fn expired_request_cannot_be_cancelled_as_if_still_pending() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4f";
        hub.propose_execution(execution_request(
            request_id,
            "idem-expired-cancel",
            "1970-01-01T00:00:00.000000001Z",
        ));
        set_predecessor_at("operator.near", 1);
        hub.cancel_execution(request_id.to_owned());

        let expired = hub
            .get_execution_request(request_id.to_owned())
            .expect("expired request remains auditable");
        assert_eq!(expired.status, ExecutionStatus::Expired);
    }

    #[test]
    fn only_proposer_or_administrator_can_cancel_execution() {
        let mut hub = initialized_hub();
        registered_project(&mut hub);
        configure_execution_policy(&mut hub, "policy-v1");
        set_predecessor("operator.near");
        let request_id = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c4e";
        hub.propose_execution(execution_request(
            request_id,
            "idem-cancel",
            "2035-01-01T00:00:00Z",
        ));
        set_predecessor(STRANGER);
        let unauthorized = catch_unwind(AssertUnwindSafe(|| {
            hub.cancel_execution(request_id.to_owned())
        }));
        assert!(unauthorized.is_err());
        set_predecessor("operator.near");
        hub.cancel_execution(request_id.to_owned());

        let cancelled = hub
            .get_execution_request(request_id.to_owned())
            .expect("cancelled request remains auditable");
        assert_eq!(cancelled.status, ExecutionStatus::Cancelled);
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
        assert_eq!(policy.operators, vec![account("operator.near")]);
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
                vec![account("operator.near")],
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
                vec![account("operator.near")],
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
