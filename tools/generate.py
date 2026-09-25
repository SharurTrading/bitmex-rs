#!/usr/bin/env python3
"""Generate typed REST contracts from the reviewed, pinned page snapshot."""

from __future__ import annotations

import argparse
import hashlib
import json
import keyword
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = json.loads((ROOT / "spec/official/rest.json").read_text())
RUST_KEYWORDS = set(keyword.kwlist) | {
    "async", "await", "dyn", "ref", "type", "self", "Self", "super", "crate", "move", "mod", "impl", "trait", "where", "loop", "match", "use", "pub", "in", "as", "const", "static", "enum", "struct", "fn", "let", "mut", "box", "try", "gen",
}


def snake(value: str) -> str:
    value = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", value)
    value = re.sub(r"[^A-Za-z0-9]+", "_", value).strip("_").lower()
    if not value or value[0].isdigit():
        value = "field_" + value
    if value in RUST_KEYWORDS:
        value += "_"
    return value


def pascal(value: str) -> str:
    result = "".join(x[:1].upper() + x[1:] for x in snake(value).split("_"))
    return result or "Value"


def distinct_field(wire: str, used: set[str]) -> str:
    field = snake(wire)
    if field in used and wire.endswith("[]"):
        field += "_list"
    suffix = 2
    candidate = field
    while candidate in used:
        candidate = f"{field}_{suffix}"
        suffix += 1
    used.add(candidate)
    return candidate


def operation_name(op: dict) -> str:
    slug = op["source"].split("/")[-1].removesuffix(".html")
    name = snake(slug)
    if op["path"].startswith("/api/v2/order"):
        name = name.removesuffix("_1") + "_v2"
    elif op["path"].startswith("/api/v1/order"):
        name = name + "_v1"
    return name


class Generator:
    def __init__(self) -> None:
        self.defs: list[str] = []
        self.named: dict[str, str] = {}
        self.names: set[str] = set()

    def unique(self, name: str) -> str:
        candidate = pascal(name)
        if candidate in self.names:
            candidate += hashlib.sha256(name.encode()).hexdigest()[:7].upper()
        self.names.add(candidate)
        return candidate

    def rust_type(self, schema: dict | None, name: str) -> str:
        schema = schema or {}
        kind = schema.get("type")
        if kind == "array":
            return f"Vec<{self.rust_type(schema.get('items'), name + 'Item')}>"
        if kind == "object" or "properties" in schema:
            properties = schema.get("properties")
            if not properties:
                return "crate::UnknownObject"
            normalized = {key: value for key, value in schema.items() if key not in ("description", "title", "example")}
            fingerprint = json.dumps(normalized, sort_keys=True)
            if fingerprint in self.named:
                existing = self.named[fingerprint]
                alias = pascal(name)
                if alias != existing and alias not in self.names:
                    self.names.add(alias)
                    self.defs.append(f"/// Operation-specific name for a shared BitMEX object contract.\npub type {alias} = {existing};\n")
                    return alias
                return existing
            model = self.unique(name)
            self.named[fingerprint] = model
            required = set(schema.get("required", []))
            fields = []
            used_fields: set[str] = set()
            has_secret = False
            for wire, child in properties.items():
                field = distinct_field(wire, used_fields)
                has_secret |= field in {"secret", "password", "api_secret", "api_key", "access_token", "refresh_token"}
                child_type = self.rust_type(child, model + pascal(wire))
                optional = wire not in required or child.get("nullable", False)
                actual_type = f"Option<{child_type}>" if optional else child_type
                attrs = [f'    #[serde(rename = "{wire}")]']
                if optional:
                    attrs.append('    #[serde(default, skip_serializing_if = "Option::is_none")]')
                if child_type == "rust_decimal::Decimal":
                    module = "optional" if optional else "value"
                    attrs.append(f'    #[serde(with = "crate::decimal_wire::{module}")]')
                fields.extend(attrs)
                fields.append(f"    pub {field}: {actual_type},")
            derives = "Clone, Default, serde::Serialize, serde::Deserialize" if has_secret else "Debug, Clone, Default, serde::Serialize, serde::Deserialize"
            definition = "/// BitMEX provider object from the pinned REST contract.\n#[derive(" + derives + ")]\npub struct " + model + " {\n" + "\n".join(fields) + "\n}\n"
            if has_secret:
                definition += "impl std::fmt::Debug for " + model + " {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.write_str(\"" + model + "([REDACTED])\")\n    }\n}\n"
            self.defs.append(definition)
            return model
        if kind == "integer":
            return "i32" if schema.get("format") == "int32" else "i64"
        if kind == "number":
            return "rust_decimal::Decimal"
        if kind == "boolean":
            return "bool"
        if kind == "string":
            return "String"
        return "serde_json::Value"


def schema_for(response: dict) -> dict | None:
    content = response.get("content", {})
    return content.get("application/json", {}).get("schema")


def minimal_value(schema: dict | None):
    schema = schema or {}
    kind = schema.get("type")
    if kind == "array":
        return []
    if kind == "object" or "properties" in schema:
        required = schema.get("required", [])
        return {key: minimal_value(schema.get("properties", {}).get(key)) for key in required}
    if kind == "string":
        return (schema.get("enum") or ["fixture"])[0]
    if kind in ("integer", "number"):
        return 1
    if kind == "boolean":
        return False
    return None


def access_note(path: str, method: str) -> str | None:
    if path == "/api/v2/order/bulkorder":
        return "BitMEX documents a bespoke arrangement for contingent bulk orders"
    if path.startswith("/api/v1/broker/"):
        return "brokerage role or venue authorization required"
    if path.startswith("/api/v1/managedSubAccountBinding/"):
        return "managed-subaccount access is venue and account dependent"
    if method != "GET" and path.startswith("/api/v1/user/"):
        return "API-key access to account settings and withdrawal actions is restricted by BitMEX; verify venue permissions"
    return None


def emit() -> tuple[str, str, list[dict]]:
    gen = Generator()
    methods = []
    ledger = []
    seen_names = set()
    for op in SPEC["operations"]:
        name = operation_name(op)
        if name in seen_names:
            name += "_" + hashlib.sha256(op["path"].encode()).hexdigest()[:6]
        seen_names.add(name)
        stem = pascal(name)
        response_schema = schema_for(op["responses"]["200"])
        blocked = response_schema is None or (response_schema.get("type") == "object" and not response_schema.get("properties"))
        result_type = gen.rust_type(response_schema, stem + "Response") if not blocked else None
        args = []
        query_fields = []
        used_query_fields: set[str] = set()
        for param in op["parameters"]:
            wire = param["name"]
            if param.get("in") == "path":
                args.append((snake(wire), "&crate::PathId"))
                continue
            child = param.get("schema", {})
            child_type = gen.rust_type(child, stem + pascal(wire))
            optional = not param.get("required", False)
            actual_type = f"Option<{child_type}>" if optional else child_type
            query_fields.append(f'    #[serde(rename = "{wire}")]')
            if optional:
                query_fields.append('    #[serde(skip_serializing_if = "Option::is_none")]')
            if child_type == "rust_decimal::Decimal":
                query_fields.append(f'    #[serde(with = "crate::decimal_wire::{"optional" if optional else "value"}")]')
            query_fields.append(f"    pub {distinct_field(wire, used_query_fields)}: {actual_type},")
        query_name = None
        if query_fields:
            query_name = stem + "Query"
            gen.defs.append("/// Query for the corresponding BitMEX REST operation.\n#[derive(Debug, Clone, Default, serde::Serialize)]\npub struct " + query_name + " {\n" + "\n".join(query_fields) + "\n}\n")
            args.append(("query", f"&{query_name}"))
        body_name = None
        body = op["request_body"]
        if body:
            body_schema = body.get("content", {}).get("application/json", {}).get("schema")
            if body_schema:
                body_name = gen.rust_type(body_schema, stem + "Body")
                args.append(("body", f"&{body_name}"))
        operation = {
            "method": op["method"], "path": op["path"], "source": op["source"],
            "public_method": name if not blocked else None,
            "response_type": result_type,
            "request_type": body_name,
            "query_type": query_name,
            "status": "blocked" if blocked else "implemented",
            "blocker": "published 200 response has no field contract" if blocked else None,
            "access": access_note(op["path"], op["method"]),
        }
        ledger.append(operation)
        if blocked:
            continue
        path_expr = '"' + op["path"] + '".to_owned()'
        for param in op["parameters"]:
            if param.get("in") == "path":
                path_expr = path_expr.replace(":" + param["name"], "{" + snake(param["name"]) + "}")
        if any(p.get("in") == "path" for p in op["parameters"]):
            path_expr = 'format!("' + path_expr.split('"')[1] + '", ' + ', '.join(f"{n} = {n}.encoded()" for n, _ in args if n != "query" and n != "body") + ')'
        is_mutation = op["method"] != "GET"
        q = "Some(query)" if query_name else "None::<&()>"
        b = "Some(body)" if body_name else "None::<&()>"
        arg_list = ", ".join(f"{n}: {t}" for n, t in args)
        method = f'''    /// {op["title"].rstrip('.')}.\n    /// Source: <{op["source"]}>\n    pub async fn {name}(&self{", " if arg_list else ""}{arg_list}) -> Result<crate::ApiResponse<{result_type}>, crate::OperationError<crate::ProviderRejection>> {{\n        let path = {path_expr};\n        self.execute(reqwest::Method::{op["method"]}, &path, {q}, {b}, {str(is_mutation).lower()}).await\n    }}\n'''
        methods.append(method)
    models = "// @generated by tools/generate.py from spec/official/rest.json. Do not edit.\n#![allow(missing_docs, clippy::pedantic)]\n" + "\n".join(gen.defs)
    api = "// @generated by tools/generate.py from spec/official/rest.json. Do not edit.\n#![allow(missing_docs, clippy::pedantic)]\nuse crate::generated::models::*;\nimpl crate::Client {\n" + "\n".join(methods) + "}\n"
    return models, api, ledger


def emit_tests() -> str:
    tests = ["//! Generated, deterministic REST loopback fixtures.\n// @generated by tools/generate.py. Do not edit.\n#![allow(clippy::expect_used)]\nmod support;\nuse bitmex_client::{ApiCredentials, Client, Environment, PathId};\nuse bitmex_client::generated::models::*;\n"]
    for op in SPEC["operations"]:
        name = operation_name(op)
        response_schema = schema_for(op["responses"]["200"])
        if response_schema is None or (response_schema.get("type") == "object" and not response_schema.get("properties")):
            continue
        stem = pascal(name)
        args = []
        setup = []
        path = op["path"]
        for param in op["parameters"]:
            if param.get("in") == "path":
                var = snake(param["name"])
                setup.append(f'    let {var} = PathId::new("fixture").expect("path id");')
                args.append("&" + var)
                path = path.replace(":" + param["name"], "fixture")
        if any(p.get("in") == "query" for p in op["parameters"]):
            setup.append(f"    let query = {stem}Query::default();")
            args.append("&query")
        body = op["request_body"]
        if body:
            body_schema = body.get("content", {}).get("application/json", {}).get("schema")
            if body_schema:
                body_value = minimal_value(body_schema)
                if op["path"] in ("/api/v1/order", "/api/v2/order"):
                    if op["method"] == "POST":
                        body_value.update({"side": "Buy", "ordType": "Limit", "orderQty": 1, "price": 1})
                    elif op["method"] == "PUT":
                        body_value["orderID"] = "fixture"
                    elif op["method"] == "DELETE":
                        body_value["orderID"] = ["fixture"]
                body_json = json.dumps(body_value, separators=(",", ":"))
                # Model reuse can assign a prior name. Infer the generated alias from
                # the ledger after generation instead of assuming the stem.
                ledger = json.loads((ROOT / "docs/coverage.json").read_text())
                row = next(r for r in ledger["operations"] if r["method"] == op["method"] and r["path"] == op["path"])
                body_type = row["request_type"]
                setup.append(f'    let body: {body_type} = serde_json::from_str(r#"{body_json}"#).expect("body fixture");')
                args.append("&body")
        response_json = json.dumps(minimal_value(response_schema), separators=(",", ":"))
        test_name = name
        call = f"client.{name}({', '.join(args)}).await"
        tests.append(f'''#[tokio::test]\nasync fn {test_name}_success_and_rejection() {{\n{chr(10).join(setup)}\n    let (url, received) = support::serve(200, r#"{response_json}"#).await;\n    let client = Client::builder(Environment::Testnet)\n        .credentials(ApiCredentials::new("fixture-key", "fixture-secret").expect("credentials"))\n        .loopback_rest_url(url).build().expect("client");\n    let result = {call};\n    assert!(result.is_ok(), "success fixture for {op['method']} {path}: {{result:?}}");\n    let request = received.await.expect("request recorded");\n    assert!(request.starts_with("{op['method']} {path}"), "{{request}}");\n    assert!(request.contains("api-signature:"), "{{request}}");\n\n    let (url, received) = support::serve(400, r#"{{"error":{{"name":"ValidationError","message":"fixture"}}}}"#).await;\n    let client = Client::builder(Environment::Testnet)\n        .credentials(ApiCredentials::new("fixture-key", "fixture-secret").expect("credentials"))\n        .loopback_rest_url(url).build().expect("client");\n    let result = {call};\n    assert!(matches!(result, Err(bitmex_client::OperationError::Rejected {{ status: 400, .. }})), "rejection fixture: {{result:?}}");\n    let request = received.await.expect("request recorded");\n    assert!(request.starts_with("{op['method']} {path}"), "{{request}}");\n}}\n''')
    return "\n".join(tests)


def rustfmt(source: str) -> str:
    result = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"], input=source, text=True, capture_output=True, check=True)
    return result.stdout


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    models, api, ledger = emit()
    outputs = {
        ROOT / "src/generated/models.rs": rustfmt(models),
        ROOT / "src/generated/api.rs": rustfmt(api),
        ROOT / "docs/coverage.json": json.dumps({"reviewed_at": SPEC["reviewed_at"], "authority": SPEC["authority"], "operations": ledger}, indent=2) + "\n",
    }
    for path, value in outputs.items():
        if args.check:
            if not path.exists() or path.read_text() != value:
                raise SystemExit(f"generated file differs: {path}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(value)
    tests = rustfmt(emit_tests())
    test_path = ROOT / "tests/generated_rest.rs"
    if args.check:
        if not test_path.exists() or test_path.read_text() != tests:
            raise SystemExit(f"generated file differs: {test_path}")
    else:
        test_path.parent.mkdir(parents=True, exist_ok=True)
        test_path.write_text(tests)
    print(f"generated {sum(o['status'] == 'implemented' for o in ledger)} callable REST operations; {sum(o['status'] == 'blocked' for o in ledger)} documentation blockers")


if __name__ == "__main__":
    main()
