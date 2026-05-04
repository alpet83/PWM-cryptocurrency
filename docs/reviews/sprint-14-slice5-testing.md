# Sprint 14 — Slice 5 (testing): auto v2->v3 migration on load

## Scope

- `crates/pwm-cli/src/wallet.rs`: validation of tests around `load_wallet_yaml` auto-migration path.
- Regression smoke for existing wallet tests inside `cargo test -p pwm-cli`.

## Commands and results

1) Full crate run:

- Command: `cargo test -p pwm-cli`
- Result: **PASS**
- Totals: `103 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Duration: `60.34s`

2) Focused migration tests:

- Command: `cargo test -p pwm-cli load_wallet_yaml_auto_migrates`
- Result: **PASS**
- Totals: `2 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out`
- Duration: `13.42s`

## Migration test evidence

- `wallet::tests::load_wallet_yaml_auto_migrates_plaintext_v2_to_v3` — passed.
- `wallet::tests::load_wallet_yaml_auto_migrates_encrypted_v2_to_v3` — passed.

Both tests confirm:

- load from v2 succeeds and rewrites wallet file to schema v3 on disk;
- plaintext mode keeps plaintext contract;
- encrypted mode keeps encrypted contract (unlock with passphrase works; plaintext secret fields are not unexpectedly populated).

## Regression check (wallet tests)

- No failing wallet tests observed in full `pwm-cli` suite.
- No obvious regression found in existing wallet-related tests (`wallet::tests::*`).
- One long-running existing test (`tx_send_recipient_book_tempfile_rejects_unknown_then_allows_after_append`) completed successfully; no hang/failure.

## Bugs / regressions found

- None found in this independent testing pass.
