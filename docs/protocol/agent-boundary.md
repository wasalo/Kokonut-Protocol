# Agent execution proposal boundary

`specs/agent-execution-proposal.schema.json` defines a proposal envelope for an agent to submit an `ExecutionRequest` for independent review. The example in `specs/examples/agent-execution-proposal.json` is illustrative only.

The envelope deliberately has no approval, signature, or execution-status fields; unknown fields are rejected. Its embedded request is validated against the canonical execution-request schema. Passing schema validation means only that the object is well-formed. It does not authenticate `proposed_by`, establish that evidence is true, approve the action, satisfy NEAR policy, or authorize a signature or destination transaction.

## Required authority boundary

- Treat all agent-generated proposals and their rationale as untrusted input.
- Keep proposal submission separate from human/DAO approval. An agent must not approve its own proposal or mutate the approved action after approval.
- Apply identity, evidence, project status, destination, operation, budget, and expiry checks in the trusted approval and policy path; this schema does not perform those checks.
- Only the existing NEAR Hub approval and policy controls may authorize a signing request. Only the destination chain can authorize and finalize its local effects.
- Kokonut Intelligence remains canonical for operational records, evidence, finance, and human approval. This Protocol schema creates no KI record or approval.
- Do not treat the illustrative sample, proposal status, rationale, or a valid JSON Schema result as authorization to sign, broadcast, deploy, or settle.

No autonomous agent execution path is implemented by this schema. A future consumer must enforce the authority boundary independently and receive separate review before it can submit or execute proposals.
