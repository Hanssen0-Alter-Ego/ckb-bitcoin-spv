//! Data-format tests only: no chain updates or proof verification.
use ckb_bitcoin_spv_verifier::types::{core, packed, prelude::*};

#[test]
fn uint32_has_little_endian_layout() {
    for value in [0, 1, 0x12345678, u32::MAX] {
        let encoded: packed::Uint32 = value.pack();
        assert_eq!(encoded.as_slice(), value.to_le_bytes());
        let decoded: u32 = encoded.unpack();
        assert_eq!(decoded, value);
        let reader_decoded: u32 = encoded.as_reader().unpack();
        assert_eq!(reader_decoded, value);
    }
}

#[test]
fn uint256_preserves_each_byte() {
    let bytes: Vec<u8> = (0..32).collect();
    let value = core::U256::from_little_endian(&bytes);
    let encoded: packed::Uint256 = value.pack();
    assert_eq!(encoded.as_slice(), bytes);
    let decoded: core::U256 = encoded.unpack();
    assert_eq!(decoded, value);
}

#[test]
fn hash_preserves_raw_byte_order() {
    let bytes = std::array::from_fn(|i| i as u8);
    let value = core::Hash::from_byte_array(bytes);
    let encoded: packed::Hash = value.pack();
    assert_eq!(encoded.as_slice(), bytes);
    let decoded: core::Hash = encoded.unpack();
    assert_eq!(decoded, value);
}

#[test]
fn byte_vectors_have_length_prefix_and_payload() {
    for bytes in [vec![], vec![7], vec![0, 1, 255, 42]] {
        let value = core::Bytes::from(bytes.clone());
        let encoded: packed::Bytes = value.pack();
        let mut expected = (bytes.len() as u32).to_le_bytes().to_vec();
        expected.extend(&bytes);
        assert_eq!(encoded.as_slice(), expected);
        let decoded: core::Bytes = encoded.unpack();
        assert_eq!(decoded, value);
    }
}

#[test]
fn info_preserves_client_id() {
    for id in [0, 1, 42, 255] {
        let encoded: packed::SpvInfo = core::SpvInfo { tip_client_id: id }.pack();
        assert_eq!(encoded.as_slice(), &[id]);
        let decoded: core::SpvInfo = encoded.unpack();
        assert_eq!(decoded.tip_client_id, id);
    }
}

#[test]
fn type_args_preserve_layout_and_fields() {
    let value = core::SpvTypeArgs {
        type_id: core::Hash::from_byte_array([0x42; 32]),
        clients_count: 3,
        flags: 0x80,
    };
    let encoded: packed::SpvTypeArgs = value.pack();
    let mut expected = vec![0x42; 32];
    expected.extend([3, 0x80]);
    assert_eq!(encoded.as_slice(), expected);
    let decoded: core::SpvTypeArgs = encoded.unpack();
    assert_eq!(decoded.type_id, value.type_id);
    assert_eq!(decoded.clients_count, value.clients_count);
    assert_eq!(decoded.flags, value.flags);
}

#[test]
fn header_encoding_matches_bitcoin_consensus() {
    let header = bitcoin::blockdata::constants::genesis_block(bitcoin::Network::Bitcoin).header;
    let encoded: packed::Header = header.pack();
    assert_eq!(encoded.as_slice(), bitcoin::consensus::serialize(&header));
    assert_eq!(encoded.as_slice().len(), 80);
}

#[test]
fn header_vector_preserves_element_order() {
    let main = bitcoin::blockdata::constants::genesis_block(bitcoin::Network::Bitcoin).header;
    let test = bitcoin::blockdata::constants::genesis_block(bitcoin::Network::Testnet).header;
    let encoded: packed::HeaderVec = vec![main, test].pack();
    assert_eq!(encoded.len(), 2);
    assert_eq!(
        encoded.get(0).unwrap().as_slice(),
        bitcoin::consensus::serialize(&main)
    );
    assert_eq!(
        encoded.get(1).unwrap().as_slice(),
        bitcoin::consensus::serialize(&test)
    );
    let empty: packed::HeaderVec = Vec::<core::Header>::new().pack();
    assert!(empty.is_empty());
}
