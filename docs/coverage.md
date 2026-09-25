# BitMEX contract and coverage

The current [BitMEX REST endpoint pages](https://docs.bitmex.com/api-explorer) are the REST authority. An inventory of the English pages on 2026-09-25 found 141 distinct method/path operations. The [API Explorer Swagger](https://www.bitmex.com/api/explorer/swagger.json) has 120 operations and omits current v2 order routes; it is retained as a normalized cross-check in `spec/official/explorer-operations.json`. The [JSON WebSocket guide](https://www.bitmex.com/app/wsAPI) supplies the 30-topic inventory in `spec/official/ws-topics.json`.

`spec/official/rest.json` contains the pinned wire facts extracted from the current pages: methods, paths, parameter and JSON schema structure, response statuses, source URLs, and source hashes. It omits page prose and examples. `tools/fetch_contract.py` is an explicit online refresh; `tools/generate.py --check` and `tools/check_coverage.py` are offline CI gates. Generated source is checked in and not hand-edited.

The ledger at `docs/coverage.json` records all 141 operations. **114 have callable generated typed methods and local success/rejection fixtures. 27 are documentation-blocked** because their published `200` response is only an empty `object` with no fields. Several of these likely return nonempty data. The crate does not invent their response shapes or count them as callable. The v2 contingent bulk route also requires a bespoke BitMEX arrangement and is one of the blocked rows. Each blocker is named in the ledger.

The ledger's `access` notes mark broker, managed-subaccount, and account-setting or withdrawal routes where API-key permissions or venue eligibility can limit use. A typed method documents the wire contract; it does not grant access to the route. BitMEX's [API key guidance](https://www.bitmex.com/app/apiKeysUsage) describes account-setting restrictions.

An authenticated, read-only Testnet probe on 2026-09-25 exposed one response drift in
`GET /api/v1/apiKey/self`: the current page schema declares `cidrs` and `permissions` as arrays
of unspecified objects and `secret` as required. Testnet returned arrays of strings and omitted
`secret`. `ApiKeyListEntry` accepts both the observed strings and the documented objects, and
`secret` remains optional and redacted from `Debug`. The pinned source facts remain unchanged;
the generator carries this reviewed compatibility rule.

The realtime API exposes all 30 documented topic names, the two service endpoints, subscription acknowledgement, table actions, exact decimal row values, and explicit continuity gaps. Its table rows currently use a typed recursive field value rather than separate static Rust structs for every feed. The order-book helper is opt-in, bounded, and invalidates on gaps. This is **topic and wire coverage**, not a claim that all per-topic row fields have static models.

Normal CI uses local fixtures and no credentials. The ignored Testnet probes are read-only and must
be explicitly armed. On 2026-09-25, public instrument data, signed API-key self, bounded order
query, and a private order WebSocket subscription passed with a Testnet key.
`GET /api/v1/user/margin` returned HTTP 401 with that key; no account-data capability is claimed
from it. No production account has been
tested, and no live order has been submitted by this repository.
