#!/usr/bin/env python3
"""Derive tolerant Swift decoders without changing the saved server contract."""
import argparse
import copy
import json
from pathlib import Path


def client_schema(server):
    result = copy.deepcopy(server)

    def schema(value):
        if not isinstance(value, dict):
            return
        # Omission ignores extras. `true` would retain and re-encode them on PUT.
        if value.get("additionalProperties") is False:
            del value["additionalProperties"]
        for key in ("properties", "patternProperties", "$defs", "definitions", "dependentSchemas"):
            for child in value.get(key, {}).values():
                schema(child)
        for key in ("items", "additionalProperties", "unevaluatedProperties", "contains",
                    "propertyNames", "not", "if", "then", "else", "additionalItems"):
            schema(value.get(key))
        for key in ("allOf", "anyOf", "oneOf", "prefixItems"):
            for child in value.get(key, []):
                schema(child)

    def document(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key == "schema":
                    schema(child)
                elif key not in ("schemas", "example", "examples") and not key.startswith("x-"):
                    document(child)
        elif isinstance(value, list):
            for child in value:
                document(child)

    for value in result.get("components", {}).get("schemas", {}).values():
        schema(value)
    document(result)
    return result


if __name__ == "__main__":
    ios = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    server = ios / "Packages/DenAPI/Contract/openapi.json"
    destination = ios / "Packages/DenAPI/Sources/DenAPI/openapi.json"
    generated = json.dumps(client_schema(json.loads(server.read_text())), indent=2) + "\n"
    if args.check:
        if destination.read_text() != generated:
            raise SystemExit("Swift client schema is stale. Run apps/ios/scripts/prepare-client-schema.py.")
    else:
        destination.write_text(generated)
    print("Swift client schema matches the saved server contract plus unknown-field tolerance.")
