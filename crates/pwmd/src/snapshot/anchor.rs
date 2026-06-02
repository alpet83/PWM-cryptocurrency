//! Genesis-anchor helpers: digest commitments and single-signer fool-guard signature.

use super::types::SnapshotGenAnchor;
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use pwm_core::crypto::blake3_32;
use pwm_core::digest;
use pwm_core::genesis::GenCfg;

pub(crate) const GEN_ANCH_V: u32 = 1;
const GEN_ANCH_TAG: &[u8] = b"PWMv0/SNAPGENANCHOR/v1";

pub(crate) fn st_root(cfg: &GenCfg) -> [u8; 32] {
    digest(&cfg.state0())
}

pub(crate) fn cfg_dig(cfg: &GenCfg) -> Result<[u8; 32], String> {
    let raw = serde_json::to_vec(cfg).map_err(|e| format!("anchor gencfg digest: {e}"))?;
    Ok(blake3_32(&raw))
}

fn anch_msg(gen_root: [u8; 32], cfg_dig: [u8; 32], blk1_hash: [u8; 32]) -> [u8; 32] {
    let mut pre = Vec::with_capacity(GEN_ANCH_TAG.len() + 32 + 32 + 32);
    pre.extend_from_slice(GEN_ANCH_TAG);
    pre.extend_from_slice(&gen_root);
    pre.extend_from_slice(&cfg_dig);
    pre.extend_from_slice(&blk1_hash);
    blake3_32(&pre)
}

pub(crate) fn mk_anch(
    cfg: &GenCfg,
    blk1_hash: [u8; 32],
    signer_idx: u32,
    sk: &SigningKey,
) -> Result<SnapshotGenAnchor, String> {
    let gen_root = st_root(cfg);
    let cfg_hash = cfg_dig(cfg)?;
    let msg = anch_msg(gen_root, cfg_hash, blk1_hash);
    let sig = sk.sign(&msg).to_bytes();
    Ok(SnapshotGenAnchor {
        schema_v: GEN_ANCH_V,
        genesis_state_root: gen_root,
        gencfg_digest: cfg_hash,
        block1_hdr_hash: blk1_hash,
        signer_prod_idx: signer_idx,
        signature: sig,
    })
}

pub(crate) fn chk_anch(
    anch: &SnapshotGenAnchor,
    cfg: &GenCfg,
    blk1_hash: [u8; 32],
) -> Result<(), String> {
    if anch.schema_v != GEN_ANCH_V {
        return Err(format!(
            "snapshot genesis_anchor schema {} != {}",
            anch.schema_v, GEN_ANCH_V
        ));
    }
    let gen_root = st_root(cfg);
    if anch.genesis_state_root != gen_root {
        return Err("snapshot genesis_anchor mismatch: genesis_state_root".into());
    }
    let cfg_hash = cfg_dig(cfg)?;
    if anch.gencfg_digest != cfg_hash {
        return Err("snapshot genesis_anchor mismatch: gencfg_digest".into());
    }
    if anch.block1_hdr_hash != blk1_hash {
        return Err("snapshot genesis_anchor mismatch: block1_hdr_hash".into());
    }
    let vk = cfg
        .vals
        .set
        .get(anch.signer_prod_idx as usize)
        .ok_or_else(|| "snapshot genesis_anchor signer_prod_idx out of range".to_string())?;
    let key = VerifyingKey::from_bytes(&vk.pubkey)
        .map_err(|e| format!("snapshot genesis_anchor verifying key: {e}"))?;
    let msg = anch_msg(
        anch.genesis_state_root,
        anch.gencfg_digest,
        anch.block1_hdr_hash,
    );
    let sig = ed25519_dalek::Signature::from_bytes(&anch.signature);
    key.verify(&msg, &sig)
        .map_err(|e| format!("snapshot genesis_anchor signature invalid: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{chk_anch, mk_anch};

    #[test]
    fn anch_sig_rt() {
        let (cfg, sks) = pwm_core::dev_net();
        let anch = mk_anch(&cfg, [7u8; 32], 0, &sks[0]).expect("anchor");
        chk_anch(&anch, &cfg, [7u8; 32]).expect("verify");
    }

    #[test]
    fn anch_gen_root_mismatch_err() {
        let (cfg, sks) = pwm_core::dev_net();
        let mut anch = mk_anch(&cfg, [7u8; 32], 0, &sks[0]).expect("anchor");
        anch.genesis_state_root[0] ^= 0x01;
        let err = chk_anch(&anch, &cfg, [7u8; 32]).expect_err("must reject mismatch");
        assert!(err.contains("genesis_state_root"));
    }
}
