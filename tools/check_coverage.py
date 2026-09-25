#!/usr/bin/env python3
"""Offline, fail-closed audit of the pinned BitMEX coverage ledger."""

import json
import re
from pathlib import Path

root = Path(__file__).resolve().parents[1]
spec = json.loads((root / "spec/official/rest.json").read_text())
ledger = json.loads((root / "docs/coverage.json").read_text())
old = json.loads((root / "spec/official/explorer-operations.json").read_text())
ws = json.loads((root / "spec/official/ws-topics.json").read_text())
api = (root / "src/generated/api.rs").read_text()
tests = (root / "tests/generated_rest.rs").read_text()
feed_source = (root / "src/realtime/types.rs").read_text()

assert len(spec["operations"]) == 141, "current REST inventory changed; review source drift"
assert len(old["operations"]) == 120, "older explorer inventory changed; review drift"
assert len(ledger["operations"]) == len(spec["operations"])
assert len({(o["method"], o["path"]) for o in spec["operations"]}) == len(spec["operations"])
assert {(o["method"], o["path"]) for o in ledger["operations"]} == {(o["method"], o["path"]) for o in spec["operations"]}

for operation in ledger["operations"]:
    key = (operation["method"], operation["path"])
    if operation["status"] == "blocked":
        assert operation["blocker"] and not operation["public_method"], f"unexplained blocker: {key}"
        continue
    assert operation["status"] == "implemented", f"invalid coverage status: {key}"
    name = operation["public_method"]
    assert name and operation["response_type"], f"missing typed contract: {key}"
    assert re.search(rf"pub async fn {re.escape(name)}\b", api), f"missing public method: {key}"
    assert re.search(rf"async fn {re.escape(name)}_success_and_rejection\b", tests), f"missing local fixture: {key}"
    assert re.search(rf"client\s*\.\s*{re.escape(name)}\s*\(", tests), f"fixture does not use public method: {key}"

topics = ws["primary_public"] + ws["primary_private"] + ws["platform"]
assert len(topics) == 30 and len(set(topics)) == 30
for topic in topics:
    assert f'"{topic}"' in feed_source, f"missing typed topic: {topic}"

callable_count = sum(o["status"] == "implemented" for o in ledger["operations"])
blocked_count = sum(o["status"] == "blocked" for o in ledger["operations"])
print(f"REST: {callable_count}/141 callable and fixture-tested; {blocked_count} documented blockers")
print("JSON WebSocket: 30/30 topic names typed")
