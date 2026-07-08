"""
Financial calculations — realistic fintech code with type violations.

The violations here are subtle: wrong numeric types, shadowed names,
conditional assignments in risk functions that may never bind.

Run:  cargo run -- check examples/finance.py
"""

from __future__ import annotations

from typing import Any, overload


# ── BSK-E0003: empty portfolio and ledger ────────────────────────────────────
_open_positions = {}  # BSK-E0003: empty dict, no annotation
_trade_log = []  # BSK-E0003: empty list, no annotation


# ── BSK-E0001/E0002: core pricing functions missing all annotations ───────────
def black_scholes(S, K, T, r, sigma):  # BSK-E0001: five untyped params
    """Call option price — classic formula."""
    import math

    d1 = (math.log(S / K) + (r + 0.5 * sigma**2) * T) / (sigma * math.sqrt(T))
    _d2 = d1 - sigma * math.sqrt(T)
    # omit N(d1)/N(d2) for brevity
    return S - K * math.exp(-r * T)  # BSK-E0002: no return type


def present_value(cash_flows, discount_rate):  # BSK-E0001: untyped params
    total = 0.0
    for i, cf in enumerate(cash_flows):
        total += cf / (1 + discount_rate) ** i
    return total  # BSK-E0002: no return type


def kelly_criterion(win_prob, win_amount, loss_amount):  # BSK-E0001
    edge = win_prob * win_amount - (1 - win_prob) * loss_amount
    return edge / win_amount  # BSK-E0002: no return type


# ── returns_compatibility: Any type with no justification ─────────────────────────────────
def execute_order(order: Any) -> Any:  # returns_compatibility ×2
    return order


# ── assignment_compatibility: currency constant assigned wrong type ─────────────────────────
BASE_CURRENCY: str = 42  # assignment_compatibility: int assigned to str
RISK_FREE_RATE: float = "0.05"  # assignment_compatibility: str assigned to float
MAX_POSITION_SIZE: int = 1_000_000.0  # assignment_compatibility: float assigned to int


# ── classes_override_2: subclass changes field type in class hierarchy ────────────────
class Instrument:
    ticker: str
    notional: float
    is_derivative: bool


class Future(Instrument):
    notional: int = 0  # classes_override_2: int overrides float


class Option(Instrument):
    is_derivative: str = "yes"  # classes_override_2: str overrides bool


# ── names_undefined: forward reference to name assigned later ──────────────────────
def get_benchmark() -> str:
    return BENCHMARK_INDEX  # names_undefined: referenced before assignment


BENCHMARK_INDEX: str = "SP500"


# ── names_unbound: VaR only assigned inside the risk branch ──────────────────────
def compute_portfolio_risk(
    returns: list[float], confidence: float, stressed: bool
) -> float:
    if stressed:
        sorted_returns = sorted(returns)
        cutoff = int(len(sorted_returns) * (1 - confidence))
        var = abs(sorted_returns[cutoff])
    return var  # names_unbound: var may be unbound


# ── overloads_consistency: unannotated params make overloads identical ───────────────────
@overload
def round_to_tick(price, tick) -> float: ...  # BSK-E0001: price, tick untyped


@overload
def round_to_tick(
    price, tick
) -> float: ...  # BSK-E0001 + overloads_consistency: duplicate


def round_to_tick(price: float, tick: int) -> float:
    return round(price / tick) * tick


# ── dict_key_hashable: list literal used as a dict key in a position record ───────────
def empty_book() -> dict[list[str], float]:
    return {["AAPL", "MSFT"]: 0.0}  # dict_key_hashable: list literal as key


# ── match_exhaustiveness: non-exhaustive match on order side ────────────────────────────
def apply_slippage(side: str, price: float, bps: float) -> float:
    match side:
        case "buy":
            return price * (1 + bps / 10_000)
        case "sell":
            return price * (1 - bps / 10_000)
    # match_exhaustiveness: no wildcard — "short", "cover", etc. fall through


# ── BSK-E0025: settlement override missing @override ────────────────────────
class BaseSettlement:
    def settle(self, amount: float, currency: str) -> str:
        return f"{amount} {currency}"


class T2Settlement(BaseSettlement):
    def settle(self, amount: float, currency: str) -> str:  # BSK-E0025
        return f"T+2: {amount} {currency}"
