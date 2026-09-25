# BitMEX Rust Client Guide

This repository contains the standalone `bitmex-client` library. Stable rule IDs are review anchors.

## Boundary and contract

- **BM-BOUNDARY-01:** Own BitMEX REST and JSON WebSocket transport, provider contracts, and protocol lifecycle. Do not depend on SharurPlatform or own strategy, routing, risk, portfolio, persistence, or UI.
- **BM-CONTRACT-01:** The pinned current BitMEX endpoint pages and JSON WebSocket documentation are authoritative. The older API Explorer Swagger is a cross-check; record drift explicitly.
- **BM-COVERAGE-01:** Every inventoried operation and topic has a public typed contract and deterministic fixture, or an explicit documentation blocker. Coverage claims are computed from the ledger. An empty provider response schema is not a typed success contract.
- **BM-DECIMAL-01:** Prices, balances, fees, rates, margins, and P&L use exact `rust_decimal::Decimal` parsing and numeric serialization without a floating-point round trip.
- **BM-ID-01:** Public provider identities are distinct validated types. Money-moving requests name their account and instrument explicitly where the provider permits.

## Transport and safety

- **BM-SECRET-01:** Callers inject API credentials. Never load, persist, log, or expose secrets in public accessors; redact `Debug`. Remote transport requires TLS. HTTP/WS plaintext is accepted only for exact loopback fixtures.
- **BM-AUTH-01:** Sign the exact sent method, path with encoded query, expiry, and serialized body using HMAC-SHA256. Never let redirects or hidden retries replay a signed request.
- **BM-RUNTIME-01:** The caller owns the Tokio runtime. No hidden runtime or blocking network operation.
- **BM-HTTP-01:** Bound response bodies and WebSocket frames; return typed errors for provider rejection, malformed input, overflow, and transport loss. Never log raw requests or headers.
- **BM-MUTATION-01:** Never automatically retry trading or account mutations. An uncertain post-send outcome fences further mutations for that account or credential scope until caller-acknowledged provider-state reconciliation.
- **BM-RATE-01:** Rate admission and provider cooldown are shared by clones. A mutation without immediate admission fails locally. Subscription and dead-man commands consume the shared budget.
- **BM-STREAM-01:** Publish partial images and deltas with explicit continuity gaps. Never silently reconnect or treat a delta as complete before its partial image. Optional order-book projections are bounded and invalidate on gaps.

## Development

- **BM-TEST-01:** CI is deterministic and credential-free. Optional live probes are ignored, explicitly armed, Testnet-only, and read-only.
- **BM-DOC-01:** Document public APIs and non-obvious venue behavior. Keep the coverage ledger, README, and generated code synchronized.
- **BM-REVIEW-01:** Review money-moving behavior, cancellation, secrets, decimal precision, new dependencies, and coverage before release. A crates.io or GitHub release requires a separate reviewed change.

## Procedure rules

- **PROC-ATTRIB:** An AI agent posting to GitHub under the operator's login — a PR description, an
  issue, a comment or review reply, an inline comment, a release, any other publication — states the
  EXACT MODEL that authored it in the artifact's own body; a footer line naming the model is the
  usual shape. The post carries the operator's identity while speaking with the agent's judgment,
  and a reader — the operator's future self, a reviewer, an auditor — is owed the distinction
  between the operator's voice and the machine's. The attribution names the model identifier the
  harness reports (e.g. `GLM-5.3`), never a generic "an AI" and never the harness or client
  standing in the model's place, and it lives in the text every reader sees: a machine-readable
  trailer the GitHub UI hides is not disclosure.
- **PROC-ISSUE-TRIAGE:** Every issue is classified when it is created, and a mis-classified one is
  corrected whenever it is touched. Four things, all mandatory: the native issue type (exactly one
  of Bug, Feature, Task), the kind label that spells it (bug / enhancement / task — a fixed 1:1
  mapping onto the type, so a reader filtering by label and a reader filtering by type see the same
  set of work), and the Priority ISSUE FIELD on the issue itself (Urgent / High / Medium / Low),
  defined once at the ORGANIZATION level. It is an issue field — not a project field, and never a
  label: the value travels with the issue instead of living on one board's item, it holds exactly
  one value that is re-set as urgency changes rather than accumulating stale ones, and a label
  would be free to disagree with it. It is read and written through the issue under ordinary repo
  scope — GET/POST on the issue's issue-field-values, with the field id from the organization's
  issue-fields — so a lane that can read the issue can read its priority, and no project scope is
  involved. The priority ladder is: Urgent — the platform is wrong about money, orders, or account
  state right now, or a live session is blocked; other work stops for it. High — it blocks the next
  live session, or the next step of an active plan. Medium — ordinary work, and the DEFAULT: an
  issue nobody has argued is urgent, high, or low is Medium, never unset. Low — polish, nits, and
  anything deferrable without loss. The fourth is the difficulty label — exactly one of
  `difficulty: hard` / `difficulty: medium` / `difficulty: easy` (operator direction 2026-09-18) —
  and it routes the issue to the class of agent that should take it, which is why it is a label
  where priority is a field: it is working state the repository's own issue list filters by, not a
  fact that must travel with the issue. The difficulty ladder is: hard — a frontier agent:
  architecture or identity refactors, money-path and reconciliation semantics, concurrency or
  lifecycle decisions, research-heavy evidence work, wide cross-crate changes. medium — a strong
  coding agent: real engineering on a bounded surface the issue itself already specifies. easy — a
  basic coding agent: mechanical, well-scoped work with a clear acceptance check; operator-only
  trackers awaiting credentials or a decision are easy when no implementation work is required.
  Difficulty is judged from the issue's own scope at creation and re-set whenever understanding
  changes. A documentation label sits beside the kind label when the work is docs, and any other
  repository label is welcome; none of those is required. An issue missing one of the four is a
  finding on the next PR that touches it, and in the tracker it is a gap the operator is asked to
  fill rather than a state to leave standing.
- **PROC-PR-TRIAGE:** A PR carries the same classification as the issue it closes, so the tracker
  and the PR list read as one body of work: exactly one kind label (bug / enhancement / task,
  matching what the diff changes), and the same Priority issue field set to its linked issue's
  value — a PR is an issue to the field API, so a reviewer reads the value from the PR itself
  rather than taking it on trust. The label is not redundant with the issue's type — native issue
  types exist on issues only, so on a PR the kind label IS the type, and a PR that omits it is
  unclassified however well its issue is labelled. The priority is INHERITED, never re-argued: a PR
  whose value differs from its issue's is a finding, and a genuine disagreement is settled on the
  issue, where the ladder lives. A PR closing no issue sets its own priority by that same ladder,
  and says in its description why it closes none. The value is never restated as prose in the
  description: one field, one home, and a copy could only be free to disagree with it.
- **PROC-REVIEW:** All work lands via PR into main. The operator manually starts the configured
  review agent for every PR; its findings cite rule IDs from this file. Every review conversation —
  inline, including outdated threads, and recommendations outside the diff in review-summary
  comments — receives an explicit disposition and is resolved before merge, whether relevant or
  adopted or not: relevant adopted findings are fixed, while irrelevant or declined findings are
  resolved with the recorded reason and require no unnecessary code. Human (operator) review is
  required for mutation and account reconciliation behavior, credential handling, decimal
  precision, and any new dependency. One FEATURE per PR (operator ruling 2026-09-02): a PR is split
  only when its verification needs separate diffs — a pure-move proof, a red-first pin that must
  land before the change it guards — or when parallel lanes need disjoint files; never by step count
  or description length. A plan's steps are the implementer's checklist, not PR boundaries. A plan
  whose groups only make sense together lands as ONE PR when the operator rules so; the split
  criteria above govern everything else.
