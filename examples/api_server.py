"""
REST API handler — realistic web service code with type violations.

Run:  cargo run -- check examples/api_server.py
"""

from __future__ import annotations

import json
from typing import Any, overload


# ── BSK-E0003: can't infer type from empty dict literal ─────────────────────
_route_table: dict[str, Any] = {}  # BSK-E0003: empty dict, no annotation
_middleware_stack: list[Any] = []  # BSK-E0003: empty list, no annotation


# ── BSK-E0001/E0002: untyped handler signatures ──────────────────────────────
def handle_get(request: Any, context: Any) -> None:  # BSK-E0001: request, context untyped
    user_id = request.get("user_id")
    return {"user": user_id}  # BSK-E0002: no return type


def handle_post(request: Any, body: Any, auth: Any) -> None:  # BSK-E0001: three untyped params
    if not auth:
        return None
    return body  # BSK-E0002: no return type


# ── BSK-E0011: naked Any in public API signature ─────────────────────────────
def serialize(value: Any) -> Any:  # BSK-E0011: Any in/out, no justification
    return json.dumps(value)


# ── BSK-E0014: int field assigned a string at module level ───────────────────
MAX_RETRIES: int = "three"  # BSK-E0014: "three" is not int
TIMEOUT_MS: int = 30.5  # BSK-E0014: float assigned to int


# ── BSK-E0017: child route overrides attribute with incompatible type ─────────
class BaseRoute:
    path: str
    method: str
    priority: int


class AdminRoute(BaseRoute):
    priority = "high"  # BSK-E0017: str overrides int


# ── BSK-E0021: overload signatures identical (both take no-annotation param) ──
@overload
def parse_id(raw: Any) -> int: ...  # BSK-E0001: raw untyped


@overload
def parse_id(raw: Any) -> int: ...  # BSK-E0001 + BSK-E0021: duplicate overload


def parse_id(raw: str) -> int:
    return int(raw)


# ── BSK-E0022: unhashable list literal as dict key ────────────────────────────
def default_routes() -> dict[list[str], str]:  # BSK-E0022 inside return annotation
    return {["GET", "POST"]: "/"}  # BSK-E0022: list literal as key


# ── BSK-E0025: override without @override decorator ─────────────────────────
class Router:
    def resolve(self, path: str) -> str:
        return path


class PrefixRouter(Router):
    prefix = "/api"

    def resolve(self, path: str) -> str:  # BSK-E0025: missing @override
        return self.p + path


# ── BSK-E0019: variable assigned inside if, returned outside ─────────────────
def extract_token(headers: dict[str, str]) -> str:
    if "Authorization" in headers:
        token = headers["Authorization"].split(" ")[-1]
    return token  # BSK-E0019: token may be unbound
