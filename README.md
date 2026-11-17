# CKB Bitcoin SPV

[![License]](#license)
[![GitHub Actions]](https://github.com/ckb-cell/ckb-bitcoin-spv/actions)

> [!WARNING]
> This repository is still in the proof-of-concept stage.

A [Bitcoin] SPV library, which is used to develop Bitcoin SPV on [CKB].

[License]: https://img.shields.io/badge/License-MIT-blue.svg
[GitHub Actions]: https://github.com/ckb-cell/ckb-bitcoin-spv/workflows/CI/badge.svg

## Resources

- [The Design of CKB Bitcoin SPV](docs/Design.md)

- [The Official Reference Implementation](https://github.com/ckb-cell/ckb-bitcoin-spv-contracts)

## License

Licensed under [MIT License].

[Bitcoin]: https://bitcoin.org
[CKB]: https://github.com/nervosnetwork/ckb

[MIT License]: LICENSE

## Independent host data tests

These targets check ordinary serialized data layouts, conversions and file IO.
They do not need the test-data submodule, a running node or a CKB VM, and do not
execute the workspace's proof-verification tests:

```sh
cargo test --locked -p ckb-bitcoin-spv-verifier --test host_data
cargo test --locked -p ckb-bitcoin-spv-prover --test host_io
```

Add `--release` to exercise the release profile. The separate **Host data and IO
tests** workflow runs both profiles. These tests supplement, not replace, the
existing workspace suite; passing them does not validate proof verification or
contract integration.
