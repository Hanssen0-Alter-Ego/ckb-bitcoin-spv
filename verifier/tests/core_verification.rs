//! Offline tests of the real verifier, not a mocked verification result.
use bitcoin::{blockdata::constants::genesis_block, consensus::serialize, Network};
use ckb_bitcoin_spv_verifier::{
    error::{BootstrapError, VerifyTxError},
    types::{core, packed, prelude::*},
};

fn fixture() -> (
    packed::SpvClient,
    packed::TransactionProof,
    core::Transaction,
) {
    let block = genesis_block(Network::Bitcoin);
    let tx = block.txdata[0].clone();
    let merkle = core::MerkleBlock::from_block_with_predicate(&block, |_| true);
    let client = packed::SpvBootstrap::new_builder()
        .height(0u32.pack())
        .header(block.header.pack())
        .build()
        .initialize_spv_client()
        .unwrap_or_else(|e| panic!("bootstrap error {}", e as i8))
        .pack();
    let proof = packed::TransactionProof::new_builder()
        .height(0u32.pack())
        .tx_index(0u32.pack())
        .transaction_proof(core::Bytes::from(serialize(&merkle)).pack())
        .build();
    (client, proof, tx)
}

fn verify(
    client: &packed::SpvClient,
    proof: &packed::TransactionProof,
    tx: &core::Transaction,
    confirmations: u32,
) -> Result<core::Header, i8> {
    client
        .verify_transaction(tx.txid().as_ref(), proof.as_reader(), confirmations)
        .map_err(|e| e as i8)
}

#[test]
fn genesis_transaction_passes_real_merkle_and_mmr_verification() {
    let (client, proof, tx) = fixture();
    assert_eq!(
        verify(&client, &proof, &tx, 0).unwrap(),
        genesis_block(Network::Bitcoin).header
    );
    let (header, decoded) = client
        .verify_transaction_data(&serialize(&tx), proof.as_reader(), 0)
        .unwrap_or_else(|e| panic!("verify error {}", e as i8));
    assert_eq!(decoded, tx);
    assert_eq!(header, genesis_block(Network::Bitcoin).header);
}

#[test]
fn confirmation_threshold_is_enforced() {
    let (client, proof, tx) = fixture();
    assert_eq!(
        verify(&client, &proof, &tx, 1),
        Err(VerifyTxError::TransactionUnconfirmed as i8)
    );
}

#[test]
fn height_outside_client_range_is_rejected() {
    let (client, proof, tx) = fixture();
    let newer = proof.clone().as_builder().height(1u32.pack()).build();
    assert_eq!(
        verify(&client, &newer, &tx, 0),
        Err(VerifyTxError::TransactionTooNew as i8)
    );
    let older_client = client
        .as_builder()
        .headers_mmr_root(
            core::HeaderDigest::new_leaf(1, &genesis_block(Network::Bitcoin).header).pack(),
        )
        .build();
    assert_eq!(
        verify(&older_client, &proof, &tx, 0),
        Err(VerifyTxError::TransactionTooOld as i8)
    );
}

#[test]
fn transaction_id_and_index_are_bound_to_merkle_matches() {
    let (client, proof, tx) = fixture();
    let bad_index = proof.clone().as_builder().tx_index(1u32.pack()).build();
    assert_eq!(
        verify(&client, &bad_index, &tx, 0),
        Err(VerifyTxError::TxOutProofInvalidTxIndex as i8)
    );
    let mut different_tx = tx;
    different_tx.output[0].value = bitcoin::Amount::from_sat(1);
    assert_eq!(
        verify(&client, &proof, &different_tx, 0),
        Err(VerifyTxError::TxOutProofInvalidTxId as i8)
    );
}

#[test]
fn truncated_transaction_and_merkle_proof_are_rejected() {
    let (client, proof, tx) = fixture();
    let bytes = serialize(&tx);
    assert!(matches!(
        client.verify_transaction_data(&bytes[..bytes.len() - 1], proof.as_reader(), 0),
        Err(VerifyTxError::DecodeTransaction)
    ));
    let bytes = proof.transaction_proof().raw_data();
    let truncated = proof
        .as_builder()
        .transaction_proof(core::Bytes::from(bytes[..bytes.len() - 1].to_vec()).pack())
        .build();
    assert_eq!(
        verify(&client, &truncated, &tx, 0),
        Err(VerifyTxError::DecodeTxOutProof as i8)
    );
}

#[test]
fn merkle_root_mismatch_is_rejected() {
    let (client, proof, tx) = fixture();
    let mut block = genesis_block(Network::Bitcoin);
    let mut merkle = core::MerkleBlock::from_block_with_predicate(&block, |_| true);
    block.txdata[0].output[0].value = bitcoin::Amount::from_sat(1);
    merkle.header.merkle_root = block.compute_merkle_root().unwrap();
    let altered = proof
        .as_builder()
        .transaction_proof(core::Bytes::from(serialize(&merkle)).pack())
        .build();
    assert_eq!(
        verify(&client, &altered, &tx, 0),
        Err(VerifyTxError::TxOutProofIsInvalid as i8)
    );
}

#[test]
fn valid_merkle_proof_must_also_belong_to_client_mmr() {
    let (client, proof, tx) = fixture();
    let mut merkle =
        core::MerkleBlock::from_block_with_predicate(&genesis_block(Network::Bitcoin), |_| true);
    merkle.header.nonce ^= 1; // Merkle membership still holds; header membership does not.
    let altered = proof
        .as_builder()
        .transaction_proof(core::Bytes::from(serialize(&merkle)).pack())
        .build();
    assert_eq!(
        verify(&client, &altered, &tx, 0),
        Err(VerifyTxError::HeaderMmrProof as i8)
    );
}

#[test]
fn bootstrap_rejects_non_boundary_height_and_invalid_pow() {
    let header = genesis_block(Network::Bitcoin).header;
    let bootstrap = packed::SpvBootstrap::new_builder()
        .height(1u32.pack())
        .header(header.pack())
        .build();
    assert!(matches!(
        bootstrap.initialize_spv_client(),
        Err(BootstrapError::Height)
    ));
    let mut invalid = header;
    invalid.bits = bitcoin::pow::CompactTarget::from_consensus(0); // zero target: no valid PoW
    let bootstrap = bootstrap
        .as_builder()
        .height(0u32.pack())
        .header(invalid.pack())
        .build();
    assert!(matches!(
        bootstrap.initialize_spv_client(),
        Err(BootstrapError::Pow)
    ));
}

#[test]
fn multiple_merkle_matches_bind_each_index_and_nonempty_mmr_path() {
    use ckb_bitcoin_spv_verifier::utilities::mmr::{lib::Merge, MergeHeaderDigest};
    // Synthetic offline block/client fixture. This tests membership, not PoW or chain admission.
    let mut block = genesis_block(Network::Bitcoin);
    let mut second = block.txdata[0].clone();
    second.output[0].value = bitcoin::Amount::from_sat(2);
    block.txdata.push(second);
    let mut third = block.txdata[0].clone();
    third.output[0].value = bitcoin::Amount::from_sat(3);
    block.txdata.push(third);
    block.header.merkle_root = block.compute_merkle_root().unwrap();
    let merkle =
        core::MerkleBlock::from_block_with_predicate(&block, |id| *id != block.txdata[1].txid());
    let leaf = core::HeaderDigest::new_leaf(100, &block.header).pack();
    let mut tip = block.header;
    tip.prev_blockhash = block.header.block_hash();
    tip.nonce ^= 1;
    let sibling = core::HeaderDigest::new_leaf(101, &tip).pack();
    let root = MergeHeaderDigest::merge(&leaf, &sibling).unwrap();
    let client = packed::SpvClient::new_builder()
        .headers_mmr_root(root)
        .build();
    let proof = packed::TransactionProof::new_builder()
        .height(100u32.pack())
        .transaction_proof(core::Bytes::from(serialize(&merkle)).pack())
        .header_proof(packed::MmrProof::new_builder().push(sibling).build())
        .build();
    for index in [0, 2] {
        let tx = &block.txdata[index];
        let selected = proof
            .clone()
            .as_builder()
            .tx_index((index as u32).pack())
            .build();
        assert_eq!(verify(&client, &selected, tx, 1).unwrap(), block.header);
        assert_eq!(
            verify(&client, &selected, tx, 2),
            Err(VerifyTxError::TransactionUnconfirmed as i8)
        );
        assert_eq!(
            verify(&client, &selected, &block.txdata[2 - index], 1),
            Err(VerifyTxError::TxOutProofInvalidTxId as i8)
        );
    }
    let unmatched = proof.clone().as_builder().tx_index(1u32.pack()).build();
    assert_eq!(
        verify(&client, &unmatched, &block.txdata[1], 1),
        Err(VerifyTxError::TxOutProofInvalidTxIndex as i8)
    );
    let missing_path = proof
        .as_builder()
        .header_proof(packed::MmrProof::default())
        .build();
    assert_eq!(
        verify(&client, &missing_path, &block.txdata[0], 1),
        Err(VerifyTxError::HeaderMmrProof as i8)
    );
}

#[test]
fn difficulty_retarget_clamps_timespan_and_network_limit() {
    use bitcoin::{blockdata::constants::DIFFCHANGE_TIMESPAN, pow::Target};
    use ckb_bitcoin_spv_verifier::{
        constants::FLAG_CHAIN_TYPE_SIGNET, utilities::bitcoin::calculate_next_target,
    };
    let target = Target::from_be_bytes({
        let mut bytes = [0u8; 32];
        bytes[10] = 16;
        bytes
    });
    let start = DIFFCHANGE_TIMESPAN * 2;
    assert_eq!(
        calculate_next_target(target, start, start + DIFFCHANGE_TIMESPAN, 0),
        target
    );
    let minimum = calculate_next_target(target, start, start + DIFFCHANGE_TIMESPAN / 4, 0);
    let mut expected_bytes = target.to_be_bytes();
    expected_bytes[10] = 4;
    assert_eq!(minimum, Target::from_be_bytes(expected_bytes));
    assert_eq!(calculate_next_target(target, start, start, 0), minimum);
    assert_eq!(calculate_next_target(target, start, start - 1, 0), minimum);
    let maximum = calculate_next_target(target, start, start + DIFFCHANGE_TIMESPAN * 4, 0);
    expected_bytes[10] = 64;
    assert_eq!(maximum, Target::from_be_bytes(expected_bytes));
    assert_eq!(
        calculate_next_target(target, start, start + DIFFCHANGE_TIMESPAN * 8, 0),
        maximum
    );
    assert_eq!(
        calculate_next_target(Target::MAX, 0, DIFFCHANGE_TIMESPAN * 4, 0),
        Target::MAX
    );
    assert_eq!(
        calculate_next_target(
            Target::MAX_ATTAINABLE_SIGNET,
            0,
            DIFFCHANGE_TIMESPAN * 4,
            FLAG_CHAIN_TYPE_SIGNET
        ),
        Target::MAX_ATTAINABLE_SIGNET
    );
}

#[test]
fn fork_choice_uses_strict_total_work_not_height() {
    let (client, _, _) = fixture();
    let mut rival: core::SpvClient = client.unpack();
    rival.headers_mmr_root.max_height = 1000;
    let work = rival.headers_mmr_root.partial_chain_work;
    assert!(!rival.pack().is_better_than(&client));
    rival.headers_mmr_root.partial_chain_work = work - core::U256::one();
    assert!(client.is_better_than(&rival.pack()));
    rival.headers_mmr_root.partial_chain_work = work + core::U256::one();
    assert!(rival.pack().is_better_than(&client));
}
