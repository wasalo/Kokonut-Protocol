use near_sdk::serde_json::{self, Value};
use near_sdk::{env, near, Gas, GasWeight, NearToken, PanicOnDefault, PromiseError};

const SIGNATURE_YIELD_ID: [u8; 32] = [0x72; 32];

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct MockChainSignatures {
    last_request_json: Option<String>,
}

#[near]
impl MockChainSignatures {
    #[init]
    pub fn new() -> Self {
        Self {
            last_request_json: None,
        }
    }

    #[payable]
    pub fn sign(&mut self, request: Value) {
        self.last_request_json = Some(request.to_string());
        let promise = env::promise_yield_create_with_id(
            "resolve_signature",
            b"{}",
            NearToken::from_yoctonear(0),
            Gas::from_tgas(30),
            GasWeight(0),
            &SIGNATURE_YIELD_ID,
        )
        .unwrap_or_else(|| env::panic_str("mock signature yield already pending"));
        env::promise_return(promise);
    }

    pub fn resume_signature(&self, signature: Value) -> bool {
        env::promise_yield_resume_with_yield_id(
            &SIGNATURE_YIELD_ID,
            signature.to_string().as_bytes(),
        )
    }

    pub fn resolve_signature(
        &self,
        #[callback_result] signature: Result<Value, PromiseError>,
    ) -> Value {
        signature.unwrap_or_else(|_| env::panic_str("mock signature resume failed"))
    }

    pub fn get_last_request(&self) -> Option<Value> {
        self.last_request_json
            .as_deref()
            .and_then(|request| serde_json::from_str(request).ok())
    }
}
