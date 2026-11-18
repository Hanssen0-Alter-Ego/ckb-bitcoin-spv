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

## Tests

Run the workspace suite with `make test`. To run only the data-layout and IO
tests without the test-data submodule:

```sh
cargo test --locked -p ckb-bitcoin-spv-verifier --test host_data
cargo test --locked -p ckb-bitcoin-spv-prover --test host_io
```

Add `--release` to test optimized builds. These targets supplement, not replace,
the workspace's proof-verification tests.
