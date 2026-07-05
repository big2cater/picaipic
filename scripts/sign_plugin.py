#!/usr/bin/env python3
"""PicAiPic plugin package signing tool.

Uses Ed25519 to sign picaipic.package.json. The host verifies the signature
on install and checks the publisher's public key against the user's trust
store.

Usage:
    # Generate a new Ed25519 keypair (prints base64 private + public keys)
    python scripts/sign_plugin.py generate-key

    # Sign a package manifest in-place (adds a "signature" field)
    python scripts/sign_plugin.py sign <path-to-picaipic.package.json> <private-key-base64>

Requirements:
    pip install cryptography
"""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path


def cmd_generate_key() -> int:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    from cryptography.hazmat.primitives.serialization import (
        Encoding,
        PrivateFormat,
        PublicFormat,
        NoEncryption,
    )

    private_key = Ed25519PrivateKey.generate()
    public_key = private_key.public_key()

    priv_bytes = private_key.private_bytes(
        encoding=Encoding.Raw,
        format=PrivateFormat.Raw,
        encryption_algorithm=NoEncryption(),
    )
    pub_bytes = public_key.public_bytes(
        encoding=Encoding.Raw,
        format=PublicFormat.Raw,
    )

    print(f"Private key (base64): {base64.b64encode(priv_bytes).decode()}")
    print(f"Public key  (base64): {base64.b64encode(pub_bytes).decode()}")
    print()
    print("Keep the private key secret. It is used only at packaging time")
    print("(`package_plugin.ps1 -SignKeyFile`) to sign picaipic.package.json;")
    print("the public key is written into the package's signature field.")
    print("On first install, users are prompted to trust the publisher by")
    print("this public key.")
    return 0


def cmd_sign(package_json_path: str, private_key_b64: str) -> int:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

    path = Path(package_json_path)
    if not path.is_file():
        print(f"Error: file not found: {path}", file=sys.stderr)
        return 1

    content = path.read_text(encoding="utf-8-sig")
    data = json.loads(content)

    # Remove any existing signature so we sign the unsigned content.
    data.pop("signature", None)

    # Canonical serialization: compact JSON with **sorted keys** and no
    # whitespace. The Rust host re-serializes via `serde_json::Value` (whose
    # default `Map` is a BTreeMap, i.e. keys in lexicographic order) then
    # `serde_json::to_vec`, producing identical bytes. Sorting on both sides
    # makes the signature independent of struct field declaration order and
    # the on-disk key order of the manifest.
    signed_bytes = json.dumps(
        data, separators=(",", ":"), ensure_ascii=False, sort_keys=True
    ).encode("utf-8")

    priv_raw = base64.b64decode(private_key_b64)
    if len(priv_raw) != 32:
        print(
            f"Error: private key must be 32 bytes (base64-decoded), got {len(priv_raw)}",
            file=sys.stderr,
        )
        return 1

    private_key = Ed25519PrivateKey.from_private_bytes(priv_raw)
    public_key = private_key.public_key()

    signature = private_key.sign(signed_bytes)

    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

    pub_bytes = public_key.public_bytes(
        encoding=Encoding.Raw,
        format=PublicFormat.Raw,
    )

    data["signature"] = {
        "algorithm": "ed25519",
        "publicKey": base64.b64encode(pub_bytes).decode(),
        "value": base64.b64encode(signature).decode(),
    }

    # Write back with pretty-printing (2-space indent) to match package_plugin.ps1 output.
    path.write_text(
        json.dumps(data, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"Signed: {path}")
    print(f"  Public key: {data['signature']['publicKey']}")
    print(f"  Signature:  {data['signature']['value'][:48]}...")
    return 0


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 1

    command = sys.argv[1]
    if command == "generate-key":
        return cmd_generate_key()
    elif command == "sign":
        if len(sys.argv) < 4:
            print("Usage: sign_plugin.py sign <package.json> <private-key-base64>", file=sys.stderr)
            return 1
        return cmd_sign(sys.argv[2], sys.argv[3])
    else:
        print(f"Unknown command: {command}", file=sys.stderr)
        print(__doc__, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
