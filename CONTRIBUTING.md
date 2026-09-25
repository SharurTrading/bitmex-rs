# Contributing

Read [AGENTS.md](AGENTS.md) before changing provider contracts, transport, or review metadata.
All changes reach `main` through a PR. Money-moving behavior, cancellation, credentials, decimal
precision, and new dependencies require human review. A release needs a separate reviewed change.

## Local checks

Use Rust 1.95.0 from [rust-toolchain.toml](rust-toolchain.toml). Normal checks need no BitMEX
credentials:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
RUSTDOCFLAGS='-D warnings' cargo doc --all-features --no-deps --locked
python3 tools/generate.py --check
python3 tools/check_coverage.py
cargo package --locked
```

The optional Testnet probe is ignored, read-only, and explicitly armed. It is not part of CI.

## Updating the BitMEX contract

The checked-in [source snapshot](spec/official/rest.json) and
[coverage ledger](docs/coverage.json) are reviewed inputs and outputs, respectively. Do not edit
generated Rust models or REST fixtures by hand. When BitMEX changes its documentation:

1. Run `python3 tools/fetch_contract.py` deliberately with network access, then inspect the
   source hashes, operation inventory, and differences from the previous pin.
2. Review authentication, order and account mutations, response schemas, and WebSocket lifecycle
   against the current official pages. Update the [coverage notes](docs/coverage.md) for drift or
   missing provider contracts. An empty success schema is a blocker, not a callable response.
3. Run `python3 tools/generate.py`, inspect the generated diff, and run every local check above.
4. Keep README coverage counts and limitations aligned with the machine-checked ledger.

The older Explorer Swagger is a cross-check; current endpoint pages take precedence. Keep
credentials, signed requests, and account data out of fixtures and issues.
