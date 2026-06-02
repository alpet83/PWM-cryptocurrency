//! Small harness helper for the V5-8 operator smoke test.
//!
//! Given a phase, batch_root, and signing material (seeds or wallet),
//! it builds a valid SignedTx for ClaimIPv4Batch and prints it as JSON.
//!
//! This is NOT a general user CLI command.

use std::path::PathBuf;

use clap::Parser;
use ed25519_dalek::SigningKey;
use pwm_core::{
    hd::{account_id_from_parts, domain_of_account_id},
    tx::{SignedTx, TxBody},
};
use serde_json::json;
use slip10_ed25519::derive_ed25519_private_key;

#[derive(Parser, Debug)]
#[command(
    name = "claim-ipv4-batch",
    about = "Build a ClaimIPv4Batch SignedTx for harness use"
)]
struct Args {
    /// Phase number (must match a phase in the genesis)
    #[arg(long)]
    phase: u8,

    /// 32-byte hex batch root (e.g. 0000...00ab)
    #[arg(long)]
    batch_root: String,

    /// Dev-only seed for the registry key (32 hex bytes). Required unless
    /// --dev-registry-is-claimant is passed.
    #[arg(long)]
    registry_seed: Option<String>,

    /// Dev-only seed for the claimant key (32 hex bytes). Required unless
    /// --wallet is passed.
    #[arg(long)]
    claimant_seed: Option<String>,

    /// Path to a demo wallet yaml (preferred for realistic smoke runs).
    #[arg(long)]
    wallet: Option<PathBuf>,

    /// Dev-only: sign registry authorization with the claimant key. Use only
    /// for smoke genesis files where registry_address == claimant.
    #[arg(long)]
    dev_registry_is_claimant: bool,

    /// Derivation index inside the wallet to use as claimant.
    #[arg(long, default_value_t = 0)]
    claimant_index: u32,

    /// Nonce to use for the claimant (must be the current on-chain nonce).
    #[arg(long, default_value_t = 0)]
    nonce: u64,

    /// Optional: path to genesis JSON (to read the phase config for validation/allocation)
    #[arg(long)]
    genesis: Option<PathBuf>,

    /// Only compute and print the claimant_id and registry without building a tx.
    /// Useful for the harness to discover the claimant before fetching the real nonce.
    #[arg(long)]
    print_claimant: bool,
}

fn main() {
    let args = Args::parse();
    validate_key_args(&args).unwrap_or_else(|e| fail(&e));

    let batch_root: [u8; 32] = hex::decode(&args.batch_root)
        .expect("invalid batch_root hex")
        .try_into()
        .expect("batch_root must be 32 bytes");

    // Resolve claimant signing key.
    // Priority: explicit seed > wallet + index
    let (claim_sk, claim_der_idx, claim_id) = if let Some(claim_seed) = &args.claimant_seed {
        let seed: [u8; 32] = hex::decode(claim_seed)
            .expect("claimant_seed must be hex")
            .try_into()
            .expect("claimant_seed must be 32 bytes");

        let key = derive_ed25519_private_key(&seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&key);
        let pk = sk.verifying_key().to_bytes();
        let id = account_id_from_parts(&pk, 0);
        (sk, 0u32, id)
    } else if let Some(wallet_path) = &args.wallet {
        // Use the same infrastructure as the rest of pwm-cli
        let source = pwm_cli::signer::load_wallet_account_signer(
            wallet_path,
            args.claimant_index,
            None,  // no passphrase for demo wallets in smoke
            false, // do not upgrade
        )
        .unwrap_or_else(|e| panic!("Failed to load claimant from wallet: {}", e));

        let claim_idx = source.derivation_index();
        let id = *source.account_id();
        let sk = source.into_signing_key();
        (sk, claim_idx, id)
    } else {
        unreachable!("validate_key_args requires --claimant-seed or --wallet")
    };

    // Resolve registry signing key.
    // In the operator smoke, the injected registry address is the demo wallet account.
    let (reg_sk, reg_id) = if let Some(reg_seed) = &args.registry_seed {
        let seed: [u8; 32] = hex::decode(reg_seed)
            .expect("registry_seed must be hex")
            .try_into()
            .expect("registry_seed must be 32 bytes");

        let key = derive_ed25519_private_key(&seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&key);
        let pk = sk.verifying_key().to_bytes();
        let id = account_id_from_parts(&pk, 0);
        (sk, id)
    } else if args.dev_registry_is_claimant {
        let sk = SigningKey::from_bytes(&claim_sk.to_bytes());
        (sk, claim_id)
    } else {
        unreachable!("validate_key_args requires --registry-seed or --dev-registry-is-claimant")
    };

    if args.print_claimant {
        let output = json!({
            "claimant_id": hex::encode(claim_id),
            "registry_address": hex::encode(reg_id),
            "phase": args.phase,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        return;
    }

    let claim_domain = domain_of_account_id(&claim_id);

    // Build the exact on-chain message
    let msg = build_claim_message(args.phase, &batch_root, &claim_id);

    // registry_sig over the claim message
    let registry_sig = pwm_core::crypto::sign(&reg_sk, &msg);

    // Build and sign the tx
    let tx = SignedTx::sign_body(
        &claim_sk,
        claim_domain,
        claim_der_idx,
        args.nonce,
        TxBody::ClaimIPv4Batch {
            phase: args.phase,
            batch_root,
            registry_sig,
        },
    );

    // Produce a convenient envelope for the PowerShell harness
    let output = json!({
        "claimant_id": hex::encode(claim_id),
        "registry_address": hex::encode(reg_id),
        "phase": args.phase,
        "batch_root": args.batch_root,
        "tx": tx
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn build_claim_message(phase: u8, batch_root: &[u8; 32], claimant: &[u8; 32]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(1 + 32 + 32 + 32);
    msg.extend_from_slice(b"PWM/IPV4/CLAIM/V1");
    msg.push(phase);
    msg.extend_from_slice(batch_root);
    msg.extend_from_slice(claimant);
    msg
}

fn validate_key_args(args: &Args) -> Result<(), String> {
    if args.claimant_seed.is_none() && args.wallet.is_none() {
        return Err(
            "set --claimant-seed or --wallet; implicit claimant test seed is disabled".into(),
        );
    }

    if args.registry_seed.is_none() && !args.dev_registry_is_claimant {
        return Err(
            "set --registry-seed or --dev-registry-is-claimant; implicit registry key fallback is disabled"
                .into(),
        );
    }

    if args.registry_seed.is_some() && args.dev_registry_is_claimant {
        return Err("use only one of --registry-seed or --dev-registry-is-claimant".into());
    }

    Ok(())
}

fn fail(msg: &str) -> ! {
    eprintln!("ERROR: {msg}");
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> Args {
        Args {
            phase: 7,
            batch_root: "00000000000000000000000000000000000000000000000000000000000000ab".into(),
            registry_seed: Some(
                "4444444444444444444444444444444444444444444444444444444444444444".into(),
            ),
            claimant_seed: Some(
                "4545454545454545454545454545454545454545454545454545454545454545".into(),
            ),
            wallet: None,
            dev_registry_is_claimant: false,
            claimant_index: 0,
            nonce: 0,
            genesis: None,
            print_claimant: false,
        }
    }

    #[test]
    fn claim_keys_need_claimant() {
        let mut args = base_args();
        args.claimant_seed = None;

        let err = validate_key_args(&args).unwrap_err();

        assert!(err.contains("--claimant-seed or --wallet"));
    }

    #[test]
    fn claim_keys_need_registry() {
        let mut args = base_args();
        args.registry_seed = None;

        let err = validate_key_args(&args).unwrap_err();

        assert!(err.contains("--registry-seed or --dev-registry-is-claimant"));
    }

    #[test]
    fn claim_keys_no_ambig() {
        let mut args = base_args();
        args.dev_registry_is_claimant = true;

        let err = validate_key_args(&args).unwrap_err();

        assert!(err.contains("use only one"));
    }
}
