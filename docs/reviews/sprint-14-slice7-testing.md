# Sprint 14 Slice 7 — testing report

Repository: `P:/opt/docker/pwm-protocol`  
Date: 2026-04-28

## Verdict

PASS (with note): Slice 7 behavior is validated by focused tests and full `cargo test -p pwm-cli` run.  
All required areas are covered; `addr-derive` deprecation semantics were additionally verified by source inspection for stderr warning + stdout compatibility.

## Exact commands run

```bash
cargo test -p pwm-cli resolve_wallet_out_path_
cargo test -p pwm-cli save_new_wallet_yaml_v3_creates_parent_directories
cargo test -p pwm-cli addr_derive_cli_keeps_stateless_mode_without_wallet_out
cargo test -p pwm-cli load_wallet_resume_start_index_for_domain_prefers_matching_cluster
cargo test -p pwm-cli addr_bruteforce_resume_start_index_prefers_target_cluster_accounts
cargo test -p pwm-cli save_new_wallet_yaml_v3_overwrites_existing_file_without_legacy_baggage
cargo test -p pwm-cli load_wallet_yaml_normalizes_legacy_pretty
cargo test -p pwm-cli
```

## Results by required check

1) Default wallet path behavior and auto mkdir  
- `resolve_wallet_out_path_defaults_to_home_wallet_file` passed.  
- `resolve_wallet_out_path_expands_tilde_prefix` passed.  
- `save_new_wallet_yaml_v3_creates_parent_directories` passed.  
- This validates default path resolution (`~/.pwm-crypto/default-wallet.yaml`), tilde expansion, and parent directory auto-create on save path.

2) `addr-derive` soft deprecation  
- `addr_derive_cli_keeps_stateless_mode_without_wallet_out` passed (backward CLI behavior without `--wallet-out`).  
- Source inspection confirms explicit deprecation warning is emitted to stderr and existing stdout fields are still printed (`wallet_path`, `wallet_write_mode`, `account_id_*`, domain/derivation/public key fields).  
- Coherence/safety: deprecated command keeps output contract while steering users to `addr-bruteforce`.

3) Cluster-aware resume  
- `load_wallet_resume_start_index_for_domain_prefers_matching_cluster` passed (mixed-domain wallet resume scoped to target cluster with fallback behavior).  
- `addr_bruteforce_resume_start_index_prefers_target_cluster_accounts` passed (CLI resume path uses cluster-aware start index).

4) `country_code_label` cleanup  
- `save_new_wallet_yaml_v3_overwrites_existing_file_without_legacy_baggage` passed (fresh saves rewrite cleanly without legacy top-level baggage).  
- `load_wallet_yaml_normalizes_legacy_pretty` passed (legacy read compatibility path intact).  
- Full suite pass (121/121) also includes wallet v2/v3 migration/read compatibility checks.

## Full suite result

- `cargo test -p pwm-cli` -> `ok`, `121 passed; 0 failed; 0 ignored`.
