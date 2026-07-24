"""
REST API handler — realistic web service code with type violations.

Run:  basilisk check examples/api_server.py
"""

from __future__ import annotations

import json
from typing import Any, overload


# ── BSK-0003: can't infer type from empty dict literal ─────────────────────
_route_table = {}  # BSK-0003: empty dict, no annotation
_middleware_stack = []  # BSK-0003: empty list, no annotation


# ── BSK-0001/0002: untyped handler signatures ──────────────────────────────
def handle_get(request, context):  # BSK-0001: request, context untyped
    user_id = request.get("user_id")
    return {"user": user_id}  # BSK-0002: no return type


def handle_post(request, body, auth):  # BSK-0001: three untyped params
    if not auth:
        return None
    return body  # BSK-0002: no return type


# ── returns_compatibility: naked Any in public API signature ─────────────────────────────
def serialize(value: Any) -> Any:  # returns_compatibility: Any in/out, no justification
    return json.dumps(value)


# ── assignment_compatibility: int field assigned a string at module level ───────────────────
MAX_RETRIES: int = "three"  # assignment_compatibility: "three" is not int
TIMEOUT_MS: int = 30.5  # assignment_compatibility: float assigned to int


# ── classes_override_2: child route overrides attribute with incompatible type ─────────
class BaseRoute:
    path: str
    method: str
    priority: int


class AdminRoute(BaseRoute):
    priority: str = "high"  # classes_override_2: str overrides int


# ── overloads_consistency: overload signatures identical (both take no-annotation param) ──
@overload
def parse_id(raw) -> int: ...  # BSK-0001: raw untyped


@overload
def parse_id(raw) -> int: ...  # BSK-0001 + overloads_consistency: duplicate overload


def parse_id(raw: str) -> int:
    return int(raw)


# ── dict_key_hashable: unhashable list literal as dict key ────────────────────────────
def default_routes() -> dict[
    list[str], str
]:  # dict_key_hashable inside return annotation
    return {["GET", "POST"]: "/"}  # dict_key_hashable: list literal as key


# ── BSK-0025: override without @override decorator ─────────────────────────
class Router:
    def resolve(self, path: str) -> str:
        return path


class PrefixRouter(Router):
    prefix: str = "/api"

    def resolve(self, path: str) -> str:  # BSK-0025: missing @override
        return self.p + path


# ── names_unbound: variable assigned inside if, returned outside ─────────────────
def extract_token(headers: dict[str, str]) -> str:
    if "Authorization" in headers:
        token = headers["Authorization"].split(" ")[-1]
    return token  # names_unbound: token may be unbound
