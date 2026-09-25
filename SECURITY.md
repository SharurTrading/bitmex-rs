# Security

Inject BitMEX API keys from your own secret store. The crate does not load credentials from environment files or persist them. Keep order and withdrawal permissions separate where possible.

Use Testnet for integration work. Validate order behavior, account scope, and provider reconciliation against your own account before production use. A network failure or cancellation after a mutation may leave its result unknown; query authoritative order, execution, wallet, and account state before acknowledging the fence.

Report security issues privately to the repository maintainers. Do not attach credentials, raw signed requests, or account data to public issues.
