# Security

Inject BitMEX API keys from your own secret store. The crate does not load credentials from environment files or persist them. Keep order and withdrawal permissions separate where possible.

BitMEX [ceased exchange trading on 23 September 2026](https://www.bitmex.com/wind-down/).
Do not treat Testnet as evidence that production order routes remain available. Login and
withdrawals remain available during wind-down, but this crate has not verified their current API
behavior. For any permitted account mutation, a network failure or cancellation may leave its
result unknown; query authoritative account and wallet state before acknowledging the fence.

Report security issues privately to the repository maintainers. Do not attach credentials, raw signed requests, or account data to public issues.
