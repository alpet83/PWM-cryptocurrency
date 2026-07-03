# ADR 0015: BFT migration gate for PWM consensus

## Status

Accepted for V7 planning. Runtime V7 continues the existing V6 incremental PoS seal path. BFT implementation is deferred to Phase 4 after this ADR is reviewed with operator devnet evidence.

## Context

PWM currently uses the V6 consensus path: `Chain::seal` remains the deterministic state transition boundary, validator admission is stake-gated, and RFC16 cluster attestation gives an intra-validator coordination layer. V7 adds devnet readiness, offchain APIs, operator UX, and the V7-1 parallel pre-processing work that raised the throughput gate target above 50 tx/s.

The roadmap separates two layers that must not be conflated:

- Intra-validator scale-up: sentry / worker / cluster infrastructure that keeps one operator fast and observable.
- Inter-validator BFT: multiple operators across WAN with Byzantine safety and finality.

This ADR decides the migration path for the inter-validator layer. It does not change `Chain::seal`, the wire protocol, or runtime behavior in V7.

## Decision

Choose path A for V7: continue incremental PoS Option A for devnet, while preparing Phase 4 for a CometBFT/ABCI integration study. Do not start a custom Rust BFT implementation by default.

CometBFT is the preferred Phase 4 candidate if PWM needs a mature inter-validator BFT engine. The Phase 4 spike must prove that ABCI integration can preserve the V7 pre-processing pipeline and keep proposal/commit latency out of the hot path. If that proof fails, PWM should explicitly defer BFT replacement and keep Option A until a narrower custom design is justified.

## Candidate paths

### A. Continue incremental PoS Option A

Pros:

- Lowest risk for the V7 devnet.
- Preserves `Chain::seal` as the single deterministic commit boundary.
- Keeps RFC16 cluster attestation and the V7 pre-processing pipeline intact.
- Lets operators validate stake admission, emergency routing, offchain anchors, and throughput before a consensus engine replacement.

Cons:

- Does not provide full Byzantine `2f+1` safety across independent operators.
- Finality remains weaker than a true BFT network.
- Requires clear operator messaging that V7 devnet is an incremental PoS devnet, not the final BFT architecture.

### B. Integrate CometBFT through an ABCI adapter

Pros:

- Mature BFT implementation, validator set handling, networking, evidence, and operational tooling.
- Avoids inventing a consensus protocol in the PWM codebase.
- Gives a clean conceptual split: CometBFT orders blocks; PWM validates and applies transactions.

Cons:

- High integration cost and API churn.
- ABCI proposal/commit flow can become the new bottleneck if PWM pushes heavy pre-validation or state cloning into consensus callbacks.
- Validator-set updates, snapshots, and offchain anchor semantics need careful mapping.
- CometBFT gossip is WAN consensus infrastructure; it is not a replacement for the intra-validator sentry/worker fast path.

### C. Custom Rust BFT replacing `Chain::seal`

Pros:

- Maximum control over data structures, batching, and pipeline integration.
- Could be shaped around PWM-specific domain and cluster assumptions.

Cons:

- Highest safety risk and implementation cost.
- Requires protocol design, formal review, adversarial testing, networking, evidence handling, and upgrade/rollback mechanics.
- Duplicates a category of software where mature implementations already exist.
- Likely delays devnet and distracts from V7 external-readiness work.

## Mandatory pipeline criterion

Any BFT path accepted for Phase 4 must preserve the V7-1 performance lesson: transaction validation and policy pre-processing must run before the final proposal/commit boundary, and the consensus path must not become a serialized CPU bottleneck.

Minimum acceptance criteria for a Phase 4 BFT spike:

- Prepared batches can be assembled from bounded queues before proposal.
- Proposal creation does not clone the full state per transaction.
- Signature and policy checks remain parallelizable where they are deterministic.
- Consensus callbacks do not perform unbounded disk or network I/O on the proposal hot path.
- The same ramp harness used in V7 can show no regression below the accepted devnet throughput gate.

CometBFT is acceptable only if the ABCI adapter can express this model without moving heavy work into `PrepareProposal`, `ProcessProposal`, or commit callbacks in a way that serializes the node.

## Chain::seal boundary contract

Preserved in V7 and during Phase 4 preparation:

- `Chain::seal` remains the canonical deterministic state transition for a prepared ordered batch.
- State mutation happens in one commit step; workers may precheck but do not own canonical state mutation.
- Existing transaction formats, policy evaluation, conservation drain, cross-shard escrow, and offchain anchors remain PWM runtime concerns.
- Snapshot replay must continue to reproduce the same state from genesis and sealed blocks.

Allowed to change in Phase 4 after a separate implementation plan:

- Who orders the batch before `Chain::seal`.
- How validator votes/finality certificates are attached to blocks.
- How validator-set changes are exported to the BFT engine.
- Whether the final commit wrapper is renamed or split, as long as the deterministic apply boundary is preserved and replayable.

Not allowed without a new ADR:

- Replacing deterministic PWM state execution with consensus-engine-specific state mutation.
- Making policy evaluation nondeterministic or dependent on wall-clock/network callbacks.
- Collapsing intra-validator sentry/worker scaling into inter-validator BFT gossip.

## RFC16 cluster compatibility

Path A preserves RFC16 unchanged. It remains the V7 mechanism for proposer/attester coordination inside the current runtime.

Path B must keep RFC16 or its successor as an intra-validator scale layer. CometBFT is for inter-validator ordering and finality, not for dynamic sentry spawn, local worker fan-in, or LAN multicast of sealed blocks.

Path C would need to define the same boundary explicitly. A custom BFT design that tries to also replace the intra-validator cluster layer is rejected as too broad for Phase 4.

## Rollback plan

If the Phase 4 CometBFT spike fails the pipeline or operator-complexity gates:

1. Keep V7 Option A as the production/devnet runtime.
2. Remove the ABCI adapter from the release branch before public operator docs reference it.
3. Keep any reusable deterministic tests, serialization fixtures, and validator-set mapping notes as research artifacts.
4. Re-open the decision with a narrower scope: either defer BFT again, or justify a custom Rust BFT ADR with explicit safety review budget.

If a deployed BFT preview network fails after launch:

1. Freeze new validator admission on the preview network.
2. Export the latest finalized PWM block and state snapshot.
3. Restart from the last agreed snapshot on the Option A runtime or a patched BFT runtime, depending on owner decision.
4. Publish a postmortem before re-opening external validators.

## Consequences

- V7 devnet remains on the V6 incremental PoS seal path.
- Phase 4 can study CometBFT without blocking devnet launch.
- Custom Rust BFT is not the default path and requires a separate ADR to become active work.
- The V7 pipeline work becomes a hard architectural constraint for any future consensus engine.
- No wire compatibility impact in V7: this ADR is documentation only.

## References

- `docs/CONCEPT_ROADMAP.md` MVP V6/V7 and R3/R13 sections
- `docs/plans/mvp_v7.md` V7-7 BFT ADR-gate section
- `docs/adr/0013-tx-pipeline-seda.md`
- `docs/adr/0014-account-hot-index-and-lockfree-chain.md`
- `docs/reviews/v7-s2-ramp-results.md`
- `docs/reviews/v7-s3-worker-scale-results.md`
