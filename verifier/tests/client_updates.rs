//! Isolated easy-target headers exercise client transitions, not Bitcoin consensus
//! admission: bootstrap deliberately trusts its supplied checkpoint/target.
use bitcoin::{blockdata::constants::genesis_block, Network};
use ckb_bitcoin_spv_verifier::{
    error::UpdateError,
    types::{core, packed, prelude::*},
    utilities::mmr::{lib::util::MemStore, ClientRootMMR},
};

fn next_header(previous: &core::Header, tag: u32) -> core::Header {
    let mut header = *previous;
    header.prev_blockhash = previous.block_hash();
    header.time += 1 + tag;
    header.nonce = 0;
    // Fixed bound prevents a broken fixture from turning into an unbounded miner.
    for nonce in 0..10000 {
        header.nonce = nonce;
        if header.validate_pow(header.target()).is_ok() {
            return header;
        }
    }
    panic!("easy-target fixture could not find a header");
}

fn checkpoint() -> packed::SpvClient {
    let header = genesis_block(Network::Regtest).header;
    packed::SpvBootstrap::new_builder()
        .height(0u32.pack())
        .header(header.pack())
        .build()
        .initialize_spv_client()
        .unwrap_or_else(|e| panic!("bootstrap {}", e as i8))
        .pack()
}

fn branch(tags: &[u32]) -> (packed::SpvClient, packed::SpvUpdate) {
    let old = checkpoint();
    let store = MemStore::default();
    let mut mmr = ClientRootMMR::new(0, &store);
    mmr.push(old.headers_mmr_root()).unwrap();
    let mut previous = genesis_block(Network::Regtest).header;
    let mut headers = Vec::new();
    let mut positions = Vec::new();
    for (index, tag) in tags.iter().enumerate() {
        let header = next_header(&previous, *tag);
        positions.push(
            mmr.push(core::HeaderDigest::new_leaf(index as u32 + 1, &header).pack())
                .unwrap(),
        );
        headers.push(header);
        previous = header;
    }
    mmr.commit().unwrap();
    let proof = mmr.gen_proof(positions).unwrap();
    let new = old
        .as_builder()
        .headers_mmr_root(mmr.get_root().unwrap())
        .tip_block_hash(core::Hash::from(previous.block_hash()).pack())
        .build();
    let update = packed::SpvUpdate::new_builder()
        .headers(headers.pack())
        .new_headers_mmr_proof(
            packed::MmrProof::new_builder()
                .set(proof.proof_items().to_vec())
                .build(),
        )
        .build();
    assert_eq!(verify(&checkpoint(), &new, &update), Ok(()));
    (new, update)
}

fn verify(
    old: &packed::SpvClient,
    new: &packed::SpvClient,
    update: &packed::SpvUpdate,
) -> Result<(), i8> {
    old.verify_new_client(new, update.clone(), 0)
        .map_err(|e| e as i8)
}

#[test]
fn valid_single_and_batch_updates_preserve_checkpoint_and_add_work() {
    let old = checkpoint();
    for count in [1usize, 2, 3, 7] {
        let (new, update) = branch(&vec![0; count]);
        assert_eq!(verify(&old, &new, &update), Ok(()));
        assert_eq!(new.headers_mmr_root().min_height().unpack(), 0u32);
        assert_eq!(new.headers_mmr_root().max_height().unpack(), count as u32);
        assert!(new.is_better_than(&old));
    }
}

#[test]
fn update_rejects_empty_discontinuous_wrong_difficulty_and_invalid_pow_headers() {
    let (new, update) = branch(&[0]);
    let old = checkpoint();
    let empty = update
        .clone()
        .as_builder()
        .headers(packed::HeaderVec::default())
        .build();
    assert_eq!(
        verify(&old, &new, &empty),
        Err(UpdateError::EmptyHeaders as i8)
    );
    let header: core::Header =
        bitcoin::consensus::deserialize(update.headers().get(0).unwrap().as_slice()).unwrap();
    for (changed, error) in [
        (
            {
                let mut h = header;
                h.prev_blockhash = genesis_block(Network::Bitcoin).block_hash();
                h
            },
            UpdateError::UncontinuousHeaders,
        ),
        (
            {
                let mut h = header;
                h.bits = bitcoin::pow::CompactTarget::from_consensus(0);
                h
            },
            UpdateError::Difficulty,
        ),
        (
            {
                let mut h = header;
                h.nonce = (0..10000)
                    .find(|nonce| {
                        h.nonce = *nonce;
                        h.validate_pow(h.target()).is_err()
                    })
                    .expect("easy-target fixture has an invalid nonce");
                h
            },
            UpdateError::Pow,
        ),
    ] {
        let changed = update
            .clone()
            .as_builder()
            .headers(vec![changed].pack())
            .build();
        assert_eq!(verify(&old, &new, &changed), Err(error as i8));
    }
}

#[test]
fn update_binds_new_client_identity_tip_and_difficulty_state() {
    let old = checkpoint();
    let (new, update) = branch(&[0, 0]);
    for (changed, error) in [
        (
            new.clone().as_builder().id(1u8.into()).build(),
            UpdateError::ClientId,
        ),
        (
            new.clone()
                .as_builder()
                .tip_block_hash(old.tip_block_hash())
                .build(),
            UpdateError::ClientTipBlockHash,
        ),
        (
            new.clone()
                .as_builder()
                .target_adjust_info(packed::TargetAdjustInfo::default())
                .build(),
            UpdateError::ClientTargetAdjustInfo,
        ),
    ] {
        assert_eq!(verify(&old, &changed, &update), Err(error as i8));
    }
}

#[test]
fn update_rejects_an_mmr_root_not_committing_to_the_appended_headers() {
    let old = checkpoint();
    let (new, update) = branch(&[0, 0]);
    let (other, _) = branch(&[9, 9]);
    let changed = new
        .as_builder()
        .headers_mmr_root(other.headers_mmr_root())
        .build();
    assert_eq!(
        verify(&old, &changed, &update),
        Err(UpdateError::HeadersMmrProof as i8)
    );
}

#[test]
fn competing_branch_is_verified_from_common_ancestor_not_appended_to_current_tip() {
    let old = checkpoint();
    let (current, _) = branch(&[0]);
    let (alternative, update) = branch(&[9, 9]);
    assert_eq!(verify(&old, &alternative, &update), Ok(()));
    assert!(alternative.is_better_than(&current));
    assert_eq!(
        verify(&current, &alternative, &update),
        Err(UpdateError::UncontinuousHeaders as i8)
    );
    let (equal_work, _) = branch(&[3]);
    assert!(!equal_work.is_better_than(&current));
    assert!(!current.is_better_than(&equal_work));
}
