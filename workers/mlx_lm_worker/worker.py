#!/usr/bin/env python3
"""Versioned Specialist Foundry worker boundary with deterministic noop only."""

from __future__ import annotations

import datetime as dt
import json
import re
import sys
from typing import Any


PROTOCOL_VERSION = 1
MAX_REQUEST_BYTES = 64 * 1024
MAX_ITERATIONS = 1_000
REQUEST_KEYS = {
    "protocol_version",
    "run_id",
    "operation",
    "requested_at",
    "total_iterations",
}
STABLE_ID = re.compile(r"[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?", re.ASCII)


class RequestError(ValueError):
    """The stdin envelope is not valid for this deliberately narrow scaffold."""


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise RequestError("duplicate JSON key")
        value[key] = item
    return value


def decode_request(payload: bytes) -> dict[str, Any]:
    if not payload or len(payload) > MAX_REQUEST_BYTES:
        raise RequestError("request size is invalid")
    try:
        decoded = json.loads(payload, object_pairs_hook=_strict_object)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise RequestError("request is not valid JSON") from error
    return validate_request(decoded)


def validate_request(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != REQUEST_KEYS:
        raise RequestError("request fields do not match protocol")
    if type(value["protocol_version"]) is not int or value["protocol_version"] != PROTOCOL_VERSION:
        raise RequestError("unsupported protocol version")
    run_id = value["run_id"]
    if not isinstance(run_id, str) or STABLE_ID.fullmatch(run_id) is None:
        raise RequestError("invalid run ID")
    if value["operation"] != "smoke_noop":
        raise RequestError("operation is unavailable in scaffold")
    iterations = value["total_iterations"]
    if type(iterations) is not int or not 1 <= iterations <= MAX_ITERATIONS:
        raise RequestError("iteration count is outside scaffold bound")
    requested_at = value["requested_at"]
    if not isinstance(requested_at, str) or len(requested_at) > 40:
        raise RequestError("invalid request timestamp")
    try:
        parsed_time = dt.datetime.fromisoformat(requested_at.replace("Z", "+00:00"))
    except ValueError as error:
        raise RequestError("invalid request timestamp") from error
    if parsed_time.tzinfo is None:
        raise RequestError("request timestamp needs a timezone")
    return value


def smoke_events(request: dict[str, Any]) -> list[dict[str, Any]]:
    common = {
        "protocol_version": PROTOCOL_VERSION,
        "run_id": request["run_id"],
    }
    return [
        {
            **common,
            "type": "started",
            "timestamp": request["requested_at"],
        },
        {
            **common,
            "type": "progress",
            "iteration": request["total_iterations"],
            "optimizer_updates": 0,
            "total_iterations": request["total_iterations"],
        },
        {
            **common,
            "type": "metric",
            "name": "smoke-check",
            "value": 1.0,
            "unit": "fraction",
            "iteration": request["total_iterations"],
        },
        {
            **common,
            "type": "completed",
            "adapter_sha256": None,
            "timestamp": request["requested_at"],
        },
    ]


def render_events(events: list[dict[str, Any]]) -> bytes:
    lines = [
        json.dumps(event, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        for event in events
    ]
    return ("\n".join(lines) + "\n").encode("ascii")


def main(argv: list[str]) -> int:
    if argv != ["--stdio"]:
        sys.stderr.write("invalid worker invocation\n")
        return 2
    try:
        payload = sys.stdin.buffer.read(MAX_REQUEST_BYTES + 1)
        request = decode_request(payload)
    except RequestError:
        sys.stderr.write("invalid worker request\n")
        return 2
    sys.stdout.buffer.write(render_events(smoke_events(request)))
    sys.stdout.buffer.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
