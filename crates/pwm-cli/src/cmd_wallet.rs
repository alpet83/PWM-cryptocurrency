//! `wallet` subcommands (`init`, `import-seed`, `show`, backup/recover, `account`, …).

use crate::bruteforce::{
    brute_force_with_policy, format_eta_human, BruteforceProgress, DomainMatchMode,
};
use crate::cli_config::resolve_wallet_out_path;
use crate::cli_parse::master_seed;
use crate::rpc_helpers::fmt_wallet_acct_line;
use crate::wallet::{
    backup_wallet_file, build_wallet_yaml, load_wallet_yaml_upgrade, recover_wallet_file,
    save_wallet_v3_new, wallet_account_add, wallet_account_list, wallet_account_remove,
    wallet_account_use, wallet_secrets,
};
use crate::wallet_shell::{
    derive_user_profile_hit, parse_domain_label_only, resolve_explicit_derivation_index,
    resolve_wallet_protection, validate_explicit_derivation_account, validate_user_profile_flags,
    wallet_reg_label, wallet_show_lines,
};
use crate::{exit_user_error, WalletAccountCmd, WalletCmd};
use pwm_core::domain_index::DomainCategory;
use pwm_core::{account_id_to_bech32dx, account_id_to_human};
use rand::RngCore;
use std::time::Instant;

pub(crate) fn run_wallet_non_book(
    wallet_passphrase: Option<String>,
    upgrade_wallet: bool,
    cmd: WalletCmd,
) {
    match cmd {
        WalletCmd::BookAdd { .. } | WalletCmd::BookList { .. } | WalletCmd::BookRemove { .. } => {
            unreachable!("address-book commands are routed via cmd_book");
        }
        WalletCmd::Init {
            country,
            master,
            max_try,
            wallet_out,
            derivation_index,
            derivation_path,
            plaintext_dev,
        } => {
            let wallet_out = resolve_wallet_out_path(wallet_out).unwrap_or_else(|e| {
                exit_user_error(&format!("failed to resolve wallet output path: {e}"))
            });
            let seed = master.map_or_else(
                || {
                    let mut s = [0u8; 32];
                    rand::thread_rng().fill_bytes(&mut s);
                    s
                },
                |m| master_seed(&m).expect("master"),
            );
            let flags_mask = 0x03FF;
            let expected_flags = 0x0000;
            validate_user_profile_flags(flags_mask, expected_flags).expect("flags policy");
            let protection = resolve_wallet_protection(wallet_passphrase.as_deref(), plaintext_dev)
                .unwrap_or_else(|e| exit_user_error(&e));
            let started = Instant::now();
            let explicit_index =
                resolve_explicit_derivation_index(derivation_index, derivation_path.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&e));
            let (hit, country_label_for_wallet) = if let Some(index) = explicit_index {
                let hit = derive_user_profile_hit(&seed, index);
                validate_explicit_derivation_account(&hit).unwrap_or_else(|e| exit_user_error(&e));
                let label = wallet_reg_label(&hit).unwrap_or_else(|| {
                    exit_user_error(
                        "derived domain has no display label after policy check (internal)",
                    )
                });
                (hit, Some(label))
            } else {
                let country_s = country
                    .as_deref()
                    .expect("clap requires --country for brute-force");
                let domain_entry = parse_domain_label_only(country_s).expect("country");
                if domain_entry.category != DomainCategory::Regulatory {
                    panic!(
                        "wallet init supports country/regulatory labels only. Label '{}' is sector/other and is rejected in this phase.",
                        domain_entry.label
                    );
                }
                let dom = domain_entry.raw as u16;
                let hit = brute_force_with_policy(
                    &seed,
                    dom,
                    DomainMatchMode::HighByteOnly,
                    flags_mask,
                    expected_flags,
                    max_try,
                    5,
                    |p: BruteforceProgress| {
                        println!(
                            "progress checked_derivations={:.4}M rate_per_sec={:.0} eta={}",
                            p.checked as f64 / 1_000_000.0,
                            p.attempts_per_sec,
                            format_eta_human(p.eta_sec)
                        );
                    },
                    |_| true,
                )
                .expect("no match");
                (hit, Some(domain_entry.label.to_string()))
            };
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let attempts = if explicit_index.is_some() {
                1
            } else {
                (hit.derivation_index as u64) + 1
            };
            let attempts_per_sec = if elapsed_ms > 0.0 {
                (attempts as f64) / (elapsed_ms / 1000.0)
            } else {
                0.0
            };
            let account_id_hex = hex::encode(hit.account_id);
            let id_pretty = account_id_to_human(&hit.account_id);
            let account_id_bech32dx = account_id_to_bech32dx(&hit.account_id);
            let country_label_print = country_label_for_wallet.clone();
            let wallet = build_wallet_yaml(
                seed,
                hit.signing_key,
                hit.verifying_key,
                hit.derivation_index,
                hit.domain,
                flags_mask,
                expected_flags,
                hit.derived_flags,
                account_id_hex.clone(),
                id_pretty.clone(),
                None,
                protection,
            )
            .unwrap_or_else(|e| exit_user_error(&format!("failed to build wallet file: {e}")));
            save_wallet_v3_new(&wallet_out, &wallet).expect("save wallet");
            println!("mode wallet_init_user_profile");
            println!("wallet_mode {}", wallet.mode);
            if let Some(ref l) = country_label_print {
                println!("country_label {}", l);
            }
            if explicit_index.is_some() {
                println!("domain_match_mode explicit_recipient_domain_policy");
            } else {
                println!("domain_match_mode high_byte_only");
            }
            println!("flags_mask_u32 {}", flags_mask);
            println!("expected_flags_u32 {}", expected_flags);
            println!("account_id_hex {}", account_id_hex);
            println!("id_pretty {}", id_pretty);
            println!("address_bech32dx {}", account_id_bech32dx);
            println!("derivation_index {}", hit.derivation_index);
            println!("derivation_path m/0/{}", hit.derivation_index);
            println!("domain_u16 {}", hit.domain);
            println!("wallet_path {}", wallet_out.display());
            println!("benchmark_attempts {}", attempts);
            println!("benchmark_elapsed_ms {:.3}", elapsed_ms);
            println!("benchmark_attempts_per_sec {:.3}", attempts_per_sec);
        }
        WalletCmd::ImportSeed {
            country,
            master,
            max_try,
            wallet_out,
            derivation_index,
            derivation_path,
            plaintext_dev,
        } => {
            let wallet_out = resolve_wallet_out_path(wallet_out).unwrap_or_else(|e| {
                exit_user_error(&format!("failed to resolve wallet output path: {e}"))
            });
            let seed = master_seed(&master)
                .unwrap_or_else(|e| exit_user_error(&format!("invalid --master seed: {e}")));
            let flags_mask = 0x03FF;
            let expected_flags = 0x0000;
            validate_user_profile_flags(flags_mask, expected_flags).expect("flags policy");
            let protection = resolve_wallet_protection(wallet_passphrase.as_deref(), plaintext_dev)
                .unwrap_or_else(|e| exit_user_error(&e));
            let started = Instant::now();
            let explicit_index =
                resolve_explicit_derivation_index(derivation_index, derivation_path.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&e));
            let (hit, country_label_for_wallet) = if let Some(index) = explicit_index {
                let hit = derive_user_profile_hit(&seed, index);
                validate_explicit_derivation_account(&hit).unwrap_or_else(|e| exit_user_error(&e));
                let label = wallet_reg_label(&hit).unwrap_or_else(|| {
                    exit_user_error(
                        "derived domain has no display label after policy check (internal)",
                    )
                });
                (hit, Some(label))
            } else {
                let country_s = country
                    .as_deref()
                    .expect("clap requires --country for brute-force");
                let domain_entry = parse_domain_label_only(country_s).expect("country");
                if domain_entry.category != DomainCategory::Regulatory {
                    panic!(
                        "wallet import-seed supports country/regulatory labels only. Label '{}' is sector/other and is rejected in this phase.",
                        domain_entry.label
                    );
                }
                let dom = domain_entry.raw as u16;
                let hit = brute_force_with_policy(
                    &seed,
                    dom,
                    DomainMatchMode::HighByteOnly,
                    flags_mask,
                    expected_flags,
                    max_try,
                    5,
                    |p: BruteforceProgress| {
                        println!(
                            "progress checked_derivations={:.4}M rate_per_sec={:.0} eta={}",
                            p.checked as f64 / 1_000_000.0,
                            p.attempts_per_sec,
                            format_eta_human(p.eta_sec)
                        );
                    },
                    |_| true,
                )
                .expect("no match");
                (hit, Some(domain_entry.label.to_string()))
            };
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let attempts = if explicit_index.is_some() {
                1
            } else {
                (hit.derivation_index as u64) + 1
            };
            let attempts_per_sec = if elapsed_ms > 0.0 {
                (attempts as f64) / (elapsed_ms / 1000.0)
            } else {
                0.0
            };
            let account_id_hex = hex::encode(hit.account_id);
            let id_pretty = account_id_to_human(&hit.account_id);
            let account_id_bech32dx = account_id_to_bech32dx(&hit.account_id);
            let country_label_print = country_label_for_wallet.clone();
            let wallet = build_wallet_yaml(
                seed,
                hit.signing_key,
                hit.verifying_key,
                hit.derivation_index,
                hit.domain,
                flags_mask,
                expected_flags,
                hit.derived_flags,
                account_id_hex.clone(),
                id_pretty.clone(),
                None,
                protection,
            )
            .unwrap_or_else(|e| exit_user_error(&format!("failed to build wallet file: {e}")));
            save_wallet_v3_new(&wallet_out, &wallet).expect("save wallet");
            println!("mode wallet_import_seed_user_profile");
            println!("wallet_mode {}", wallet.mode);
            if let Some(ref l) = country_label_print {
                println!("country_label {}", l);
            }
            if explicit_index.is_some() {
                println!("domain_match_mode explicit_recipient_domain_policy");
            } else {
                println!("domain_match_mode high_byte_only");
            }
            println!("flags_mask_u32 {}", flags_mask);
            println!("expected_flags_u32 {}", expected_flags);
            println!("account_id_hex {}", account_id_hex);
            println!("id_pretty {}", id_pretty);
            println!("address_bech32dx {}", account_id_bech32dx);
            println!("derivation_index {}", hit.derivation_index);
            println!("derivation_path m/0/{}", hit.derivation_index);
            println!("domain_u16 {}", hit.domain);
            println!("wallet_path {}", wallet_out.display());
            println!("benchmark_attempts {}", attempts);
            println!("benchmark_elapsed_ms {:.3}", elapsed_ms);
            println!("benchmark_attempts_per_sec {:.3}", attempts_per_sec);
        }
        WalletCmd::Show {
            wallet,
            unsafe_show_secrets,
        } => {
            let doc = load_wallet_yaml_upgrade(&wallet, upgrade_wallet)
                .unwrap_or_else(|e| exit_user_error(&format!("failed to read wallet file: {e}")));
            let secrets = if unsafe_show_secrets {
                Some(
                    wallet_secrets(&doc, wallet_passphrase.as_deref()).unwrap_or_else(|e| {
                        exit_user_error(&format!("failed to decode wallet secrets: {e}"))
                    }),
                )
            } else {
                None
            };
            for line in wallet_show_lines(&doc, &wallet, secrets.as_ref()) {
                println!("{line}");
            }
        }
        WalletCmd::Backup { wallet, out } => {
            if upgrade_wallet {
                load_wallet_yaml_upgrade(&wallet, true)
                    .unwrap_or_else(|e| exit_user_error(&format!("wallet upgrade failed: {e}")));
            }
            backup_wallet_file(&wallet, &out, wallet_passphrase.as_deref())
                .unwrap_or_else(|e| exit_user_error(&format!("wallet backup failed: {e}")));
            println!("mode wallet_backup");
            println!("wallet_source {}", wallet.display());
            println!("backup_path {}", out.display());
            println!("status ok");
        }
        WalletCmd::Recover { backup, out } => {
            if upgrade_wallet {
                load_wallet_yaml_upgrade(&backup, true)
                    .unwrap_or_else(|e| exit_user_error(&format!("wallet upgrade failed: {e}")));
            }
            recover_wallet_file(&backup, &out, wallet_passphrase.as_deref())
                .unwrap_or_else(|e| exit_user_error(&format!("wallet recovery failed: {e}")));
            println!("mode wallet_recover");
            println!("backup_path {}", backup.display());
            println!("wallet_restored {}", out.display());
            println!("status ok");
        }
        WalletCmd::Account { cmd } => match cmd {
            WalletAccountCmd::List { wallet } => {
                if upgrade_wallet {
                    load_wallet_yaml_upgrade(&wallet, true).unwrap_or_else(|e| {
                        exit_user_error(&format!("wallet upgrade failed: {e}"))
                    });
                }
                let accounts = wallet_account_list(&wallet).unwrap_or_else(|e| {
                    exit_user_error(&format!("wallet account list failed: {e}"))
                });
                for account in accounts {
                    println!("{}", fmt_wallet_acct_line(&account));
                }
            }
            WalletAccountCmd::Add {
                wallet,
                derivation_index,
            } => {
                if upgrade_wallet {
                    load_wallet_yaml_upgrade(&wallet, true).unwrap_or_else(|e| {
                        exit_user_error(&format!("wallet upgrade failed: {e}"))
                    });
                }
                let account =
                    wallet_account_add(&wallet, derivation_index, wallet_passphrase.as_deref())
                        .unwrap_or_else(|e| {
                            exit_user_error(&format!("wallet account add failed: {e}"))
                        });
                println!("mode wallet_account_add");
                println!("wallet_path {}", wallet.display());
                println!("id_hex {}", account.id_hex);
                println!("id_pretty {}", account.id_pretty);
                println!("derivation_index {}", account.derivation_index);
                println!("derivation_path {}", account.derivation_path);
                println!("status ok");
            }
            WalletAccountCmd::Use { wallet, id_hex } => {
                if upgrade_wallet {
                    load_wallet_yaml_upgrade(&wallet, true).unwrap_or_else(|e| {
                        exit_user_error(&format!("wallet upgrade failed: {e}"))
                    });
                }
                wallet_account_use(&wallet, &id_hex).unwrap_or_else(|e| {
                    exit_user_error(&format!("wallet account use failed: {e}"))
                });
                println!("mode wallet_account_use");
                println!("wallet_path {}", wallet.display());
                println!(
                    "selected_account_id_hex {}",
                    id_hex.trim().to_ascii_lowercase()
                );
                println!("warning deprecated_no_persisted_active_account");
                println!("status ok");
            }
            WalletAccountCmd::Remove { wallet, id_hex } => {
                if upgrade_wallet {
                    load_wallet_yaml_upgrade(&wallet, true).unwrap_or_else(|e| {
                        exit_user_error(&format!("wallet upgrade failed: {e}"))
                    });
                }
                let removed = wallet_account_remove(&wallet, &id_hex).unwrap_or_else(|e| {
                    exit_user_error(&format!("wallet account remove failed: {e}"))
                });
                println!("mode wallet_account_remove");
                println!("wallet_path {}", wallet.display());
                println!("removed_account_id_hex {}", removed.removed_id_hex);
                println!("default_account_id_hex {}", removed.new_active_id_hex);
                println!("removed_was_active {}", removed.removed_was_active);
                println!("status ok");
            }
        },
    }
}
