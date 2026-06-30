"""
REST API handler — fully typed, passes Basilisk with zero diagnostics.

Run:  cargo run -- check examples/api_server_clean.py
"""

from __future__ import annotations

import json
from typing import overload, override


# Properly annotated module-level state
_route_table: dict[str, str] = {}
_middleware_stack: list[str] = []


# Typed handler signatures
def handle_get(request: dict[str, str], context: str) -> dict[str, str]:
    user_id = request.get("user_id", "")
    return {"user": user_id}


def handle_post(
    request: dict[str, str],
    body: dict[str, str],
    auth: str,
) -> dict[str, str] | None:
    if not auth:
        return None
    return body


# Any is justified here — JSON can be any valid JSON type
def serialize(
    value: object,
) -> str:  # basilisk: allow[returns_compatibility] -- JSON is inherently untyped
    return json.dumps(value)


# Properly typed constants
MAX_RETRIES = 3
TIMEOUT_MS = 30_000


# Consistent attribute types in the hierarchy
class BaseRoute:
    path: str
    method: str
    priority: int


class AdminRoute(BaseRoute):
    priority = 100  # same type as the base class


# Non-overlapping overloads
@overload
def parse_id(raw: str) -> int: ...


@overload
def parse_id(raw: bytes) -> int: ...  # different param type


def parse_id(raw: str | bytes) -> int:
    return int(raw)


# Hashable key
def register_handler(path: str, method: str) -> None:
    _route_table[path] = method  # str is hashable


# @override present
class Router:
    def resolve(self, path: str) -> str:
        return path


class PrefixRouter(Router):
    prefix: str = "/api"

    @override
    def resolve(self, path: str) -> str:
        return self.prefix + path


# Variable always bound before use
def extract_token(headers: dict[str, str]) -> str:
    return headers.get("Authorization", "").split(" ")[-1]
