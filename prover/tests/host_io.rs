//! Ordinary binary IO tests; no SPV or VM execution.
use ckb_bitcoin_spv_prover::utilities::{decode_from_bin_file, decode_from_slice};
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

fn temp_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ckb-spv-host-io-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn slice_decodes_consensus_integer() {
    let decoded: u32 = decode_from_slice(&[0x78, 0x56, 0x34, 0x12]).unwrap();
    assert_eq!(decoded, 0x12345678);
}

#[test]
fn file_and_slice_decoding_agree() {
    let path = temp_path();
    std::fs::write(&path, [0x78, 0x56, 0x34, 0x12]).unwrap();
    let result = decode_from_bin_file::<u32, _>(&path);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(result.unwrap(), 0x12345678);
}

#[test]
fn missing_file_reports_io_failure() {
    let path = temp_path();
    assert!(!path.exists());
    assert!(decode_from_bin_file::<u32, _>(&path).is_err());
}
