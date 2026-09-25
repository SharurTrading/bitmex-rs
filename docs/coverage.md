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

The 2026-09-25 Testnet GET sweep found seven more response-shape differences from the current
endpoint pages. The generator applies the following narrow compatibility rules without altering
the pinned page snapshot:

| Route | Observed Testnet success body |
| --- | --- |
| `GET /api/v1/chat/pinned` | Empty object when the channel has no pinned message; documented fields are optional |
| `GET /api/v1/instrument/activeIntervals` | One interval object rather than an array |
| `GET /api/v1/user/commission` | Map keyed by symbol rather than an array |
| `GET /api/v1/user/csa` | Object containing a `csas` array rather than a bare array |
| `GET /api/v1/user/tradingVolume` | Array of volume objects rather than one object |
| `GET /api/v1/userEvent` | Object containing a `userEvents` array rather than a bare array |
| `GET /api/v1/wallet/currencies` | Map keyed by currency rather than an array |

The [Testnet API Explorer](https://testnet.bitmex.com/api/explorer/) and the
[Mainnet API Explorer](https://www.bitmex.com/api/explorer/) each publish 120 operations. On
2026-09-25 their operation IDs, methods, paths, parameters, and response schemas matched; the
14 differing operation descriptions only substituted Testnet links for Mainnet links. Both
older explorers include v1 `POST /api/v1/order` and omit the separately documented
[v2 `POST /api/v2/order`](https://docs.bitmex.com/api-explorer/new-order-1). Thus the Testnet
explorer is not evidence that v2 is unavailable, nor an independent up-to-date contract for it.

Read-only public comparisons showed that `instrument/activeIntervals` returned an object and
`wallet/currencies` returned a keyed map on **both** environments. Those two overrides address
published schema drift, not a known Testnet-only behavior. `chat/pinned?channelID=1` returned a
populated object on Mainnet and `{}` on Testnet; the optional-field model accepts both. The
remaining four private response shapes have only been observed with a Testnet key, so their
Mainnet behavior is unverified. BitMEX's
[Testnet terms](https://static.bitmex.com/documents/Terms_of_Service__June_2025.pdf) explicitly do not guarantee that its simulated
trading conditions or behaviors duplicate Mainnet. A Testnet response alone is therefore not
proof of Mainnet success behavior.

All 13 form-only callable operations now take a typed body, serialize it as
`application/x-www-form-urlencoded`, and sign those exact bytes. Previously the generator only
handled JSON bodies, leaving form-only route methods without their documented parameters. Five
other operations document both JSON and form bodies; those methods use their JSON contract. Local
fixtures now include nonempty array items and form content-type checks; a dedicated test verifies
percent encoding and the HMAC over the bytes actually sent.

The realtime API exposes all 30 documented topic names, the two service endpoints, subscription acknowledgement, table actions, exact decimal row values, and explicit continuity gaps. Its table rows currently use a typed recursive field value rather than separate static Rust structs for every feed. The order-book helper is opt-in, bounded, and invalidates on gaps. This is **topic and wire coverage**, not a claim that all per-topic row fields have static models.

The [WebSocket guide](https://www.bitmex.com/app/wsAPI) lists
`wss://ws.bitmex.com/realtimePlatform`, but the `ws.*` platform path returned HTTP 404 on
2026-09-25. The site-host platform paths (`wss://www.bitmex.com/realtimePlatform` and
`wss://testnet.bitmex.com/realtimePlatform`) reached the upgrade route. The guide also requires
the signature input to use `GET /realtime` for both sockets; signing `/realtimePlatform` caused
Testnet to reject the platform upgrade with HTTP 401. With the corrected host and signature,
all 30 documented topics returned subscription acknowledgements in the read-only Testnet sweep.
This validates subscription setup with the supplied key; it does not assert every table row shape
or a gap-free market-data stream.

Normal CI uses local fixtures and no credentials. The ignored Testnet probes are read-only and must
be explicitly armed. On 2026-09-25, public instrument data, signed API-key self, bounded order
query, and a private order WebSocket subscription passed with a Testnet key.
`GET /api/v1/user/margin` returned HTTP 401 with that key; no account-data capability is claimed
from it. No production account has been
tested, and no live order has been submitted by this repository.

The explicitly armed, sequential `testnet_sweep` attempted every one of the 71 callable GET
methods with the same Testnet key. With the new 64 MiB default body bound, 43 returned decoded 2xx bodies,
28 returned provider rejections (including access, input, unavailable-route, and server errors),
and none failed locally. The former 8 MiB default rejected the full unpaginated
`GET /api/v1/stats/history` response. REST bodies and WebSocket frames now default to a generous
64 MiB bound, and callers can tune each separately. The sweep is evidence for the responses this key could access, not proof
that the 28 rejected routes decode successful responses. Mutation routes remain fixture-tested
only; the live probes are read-only under `BM-TEST-01`.
