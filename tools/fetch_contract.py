#!/usr/bin/env python3
"""Capture the published BitMEX REST page contracts for offline review.

This is an explicit, online update command. Generation and CI use only the
checked-in result, and never fetch changing documentation implicitly.
"""

from __future__ import annotations

import ast
import concurrent.futures
import hashlib
import json
import re
import subprocess
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS = "https://docs.bitmex.com"
MAIN = DOCS + "/assets/js/main.b85b0a1c.js"
RUNTIME = DOCS + "/assets/js/runtime~main.da91b0bb.js"
SWAGGER = "https://www.bitmex.com/api/explorer/swagger.json"
WS = "https://www.bitmex.com/app/static/md/en-US/wsAPI"
OMIT = {"description", "example", "examples", "summary", "title", "externalDocs"}


def fetch(url: str) -> bytes:
    return subprocess.run(
        ["curl", "--fail", "--silent", "--show-error", "--location", "--max-time", "30", url],
        capture_output=True,
        check=True,
    ).stdout


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def contract_facts(value):
    """Retain wire facts, not the published prose or rendered examples."""
    if isinstance(value, dict):
        return {key: contract_facts(item) for key, item in value.items() if key not in OMIT}
    if isinstance(value, list):
        return [contract_facts(item) for item in value]
    return value


def contract(url: str, main: str, runtime: str) -> dict | None:
    page = fetch(url)
    heading = re.search(
        rb'<pre class="openapi__method-endpoint"><span[^>]*>(GET|POST|PUT|PATCH|DELETE)</span>\s*<h2[^>]*>([^<]+)</h2>',
        page,
    )
    if heading is None:
        return None
    slug = url.rstrip("/").rsplit("/", 1)[-1].removesuffix(".html")
    marker = f'@site/docs/api-explorer/{slug}.api.mdx'
    at = main.find(marker)
    if at < 0:
        raise RuntimeError(f"missing documentation module: {url}")
    chunks = re.findall(r"r\.e\((\d+)\)", main[at - 180 : at])
    if not chunks:
        raise RuntimeError(f"missing documentation chunk: {url}")
    chunk_id = chunks[-1]
    hashes = re.findall(rf"(?<!\d){chunk_id}:\"([0-9a-f]+)\"", runtime)
    if len(hashes) != 2:
        raise RuntimeError(f"missing documentation asset hashes: {url}: {hashes}")
    asset_url = f"{DOCS}/assets/js/{hashes[0]}.{hashes[1]}.js"
    asset = fetch(asset_url).decode("utf-8")
    objects = []
    for match in re.finditer(r"JSON\.parse\('((?:\\.|[^'\\])*)'\)", asset):
        objects.append(json.loads(ast.literal_eval("'" + match.group(1) + "'")))
    metadata = next((obj for obj in objects if isinstance(obj, dict) and obj.get("id") == f"api-explorer/{slug}"), None)
    responses = next((obj["responses"] for obj in objects if isinstance(obj, dict) and "responses" in obj), None)
    if metadata is None or responses is None:
        raise RuntimeError(f"incomplete published module: {url}")
    parameters = []
    request_body = None
    for obj in objects:
        if isinstance(obj, dict):
            parameters.extend(obj.get("parameters", []))
            if "body" in obj:
                request_body = obj["body"]
    return {
        "method": heading.group(1).decode(),
        "path": heading.group(2).decode(),
        "title": metadata["title"],
        "source": url,
        "source_sha256": sha256(page),
        "asset": asset_url,
        "asset_sha256": sha256(asset.encode()),
        "parameters": contract_facts(parameters),
        "request_body": contract_facts(request_body),
        "responses": contract_facts(responses),
    }


def main() -> None:
    sitemap = fetch(DOCS + "/sitemap.xml")
    urls = [
        element.text
        for element in ET.fromstring(sitemap).iter()
        if element.tag.endswith("loc") and element.text and "/api-explorer/" in element.text
    ]
    main_js, runtime_js, swagger, ws = (fetch(url) for url in (MAIN, RUNTIME, SWAGGER, WS))
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
        rows = list(pool.map(lambda url: contract(url, main_js.decode(), runtime_js.decode()), urls))
    rows = sorted((row for row in rows if row), key=lambda row: (row["path"], row["method"]))
    keys = [(row["method"], row["path"]) for row in rows]
    if len(keys) != len(set(keys)):
        raise RuntimeError("duplicate REST method/path in published documentation")
    output = {
        "reviewed_at": datetime.now(timezone.utc).date().isoformat(),
        "authority": DOCS + "/api-explorer",
        "sitemap_sha256": sha256(sitemap),
        "main_sha256": sha256(main_js),
        "runtime_sha256": sha256(runtime_js),
        "swagger_sha256": sha256(swagger),
        "ws_sha256": sha256(ws),
        "operations": rows,
    }
    dest = ROOT / "spec" / "official"
    dest.mkdir(parents=True, exist_ok=True)
    (dest / "rest.json").write_text(json.dumps(output, indent=2, sort_keys=True) + "\n")
    old = json.loads(swagger)
    old_ops = sorted((verb.upper(), old.get("basePath", "") + path) for path, methods in old["paths"].items() for verb in methods if verb.lower() in {"get", "post", "put", "patch", "delete"})
    (dest / "explorer-operations.json").write_text(json.dumps({"source": SWAGGER, "sha256": sha256(swagger), "operations": old_ops}, indent=2) + "\n")
    (dest / "ws-topics.json").write_text(json.dumps({"source": WS, "sha256": sha256(ws), "primary_public": ["funding", "instrument", "insurance", "liquidation", "orderBookL2_25", "orderBookL2", "orderBook10", "quote", "quoteBin1m", "quoteBin5m", "quoteBin1h", "quoteBin1d", "settlement", "trade", "tradeBin1m", "tradeBin5m", "tradeBin1h", "tradeBin1d"], "primary_private": ["affiliate", "execution", "order", "margin", "position", "transact", "wallet"], "platform": ["announcement", "chat", "connected", "publicNotifications", "privateNotifications"]}, indent=2) + "\n")
    print(f"captured {len(rows)} current REST operations")


if __name__ == "__main__":
    main()
