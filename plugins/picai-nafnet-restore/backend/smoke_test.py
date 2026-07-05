from __future__ import annotations

import json
import sys

from nafnet_adapter import NAFNetAdapter


if __name__ == "__main__":
    adapter = NAFNetAdapter()
    status, payload = adapter.smoke_test({
        "profileId": "manual",
        "backend": "auto",
        "capability": "denoise",
    })
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    sys.exit(0 if status < 400 else 1)
