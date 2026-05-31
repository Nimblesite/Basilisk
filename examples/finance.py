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


# ── BSK-E0011: Any type with no justification ─────────────────────────────────
def execute_order(order: Any) -> Any:  # BSK-E0011 ×2
    return order


# ── BSK-E0014: currency constant assigned wrong type ─────────────────────────
BASE_CURRENCY: str = 42  # BSK-E0014: int assigned to str
RISK_FREE_RATE: float = "0.05"  # BSK-E0014: str assigned to float
MAX_POSITION_SIZE: int = 1_000_000.0  # BSK-E0014: float assigned to int


# ── BSK-E0017: subclass changes field type in class hierarchy ────────────────
class Instrument:
    ticker: str
    notional: float
    is_derivative: bool


class Future(Instrument):
    notional: int = 0  # BSK-E0017: int overrides float


class Option(Instrument):
    is_derivative: str = "yes"  # BSK-E0017: str overrides bool


# ── BSK-E0018: forward reference to name assigned later ──────────────────────
def get_benchmark() -> str:
    return BENCHMARK_INDEX  # BSK-E0018: referenced before assignment


BENCHMARK_INDEX: str = "SP500"


# ── BSK-E0019: VaR only assigned inside the risk branch ──────────────────────
def compute_portfolio_risk(
    returns: list[float], confidence: float, stressed: bool
) -> float:
    if stressed:
        sorted_returns = sorted(returns)
        cutoff = int(len(sorted_returns) * (1 - confidence))
        var = abs(sorted_returns[cutoff])
    return var  # BSK-E0019: var may be unbound


# ── BSK-E0021: unannotated params make overloads identical ───────────────────
@overload
def round_to_tick(price, tick) -> float: ...  # BSK-E0001: price, tick untyped


@overload
def round_to_tick(price, tick) -> float: ...  # BSK-E0001 + BSK-E0021: duplicate


def round_to_tick(price: float, tick: int) -> float:
    return round(price / tick) * tick


# ── BSK-E0022: list literal used as a dict key in a position record ───────────
def empty_book() -> dict[list[str], float]:
    return {["AAPL", "MSFT"]: 0.0}  # BSK-E0022: list literal as key


# ── BSK-E0023: non-exhaustive match on order side ────────────────────────────
def apply_slippage(side: str, price: float, bps: float) -> float:
    match side:
        case "buy":
            return price * (1 + bps / 10_000)
        case "sell":
            return price * (1 - bps / 10_000)
    # BSK-E0023: no wildcard — "short", "cover", etc. fall through


# ── BSK-E0025: settlement override missing @override ────────────────────────
class BaseSettlement:
    def settle(self, amount: float, currency: str) -> str:
        return f"{amount} {currency}"


class T2Settlement(BaseSettlement):
    def settle(self, amount: float, currency: str) -> str:  # BSK-E0025
        return f"T+2: {amount} {currency}"
