"""Tests for examples/finance.py — financial calculations."""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from finance import (
    BaseSettlement,
    T2Settlement,
    apply_slippage,
    black_scholes,
    kelly_criterion,
    present_value,
    round_to_tick,
)


class TestBlackScholes:
    def test_returns_float(self) -> None:
        result = black_scholes(100, 100, 1.0, 0.05, 0.2)
        assert isinstance(result, float)

    def test_atm_option_positive(self) -> None:
        result = black_scholes(100, 100, 1.0, 0.05, 0.2)
        assert result > 0

    def test_deep_itm_approaches_intrinsic(self) -> None:
        result = black_scholes(200, 100, 0.01, 0.05, 0.2)
        assert result > 90


class TestPresentValue:
    def test_single_cash_flow(self) -> None:
        pv = present_value([100.0], 0.1)
        assert pv == 100.0

    def test_discounting_reduces_value(self) -> None:
        pv = present_value([0, 100.0], 0.1)
        assert pv < 100.0

    def test_zero_discount_rate(self) -> None:
        pv = present_value([10.0, 20.0, 30.0], 0.0)
        assert math.isclose(pv, 60.0)


class TestKellyCriterion:
    def test_fair_coin_positive_edge(self) -> None:
        fraction = kelly_criterion(0.6, 1.0, 1.0)
        assert fraction > 0

    def test_losing_bet_negative(self) -> None:
        fraction = kelly_criterion(0.3, 1.0, 1.0)
        assert fraction < 0

    def test_certain_win(self) -> None:
        fraction = kelly_criterion(1.0, 1.0, 1.0)
        assert math.isclose(fraction, 1.0)


class TestRoundToTick:
    def test_exact_multiple(self) -> None:
        assert round_to_tick(100.0, 5) == 100.0

    def test_rounds_to_nearest(self) -> None:
        result = round_to_tick(103.0, 5)
        assert result == 105.0


class TestApplySlippage:
    def test_buy_increases_price(self) -> None:
        result = apply_slippage("buy", 100.0, 10.0)
        assert result is not None
        assert result > 100.0

    def test_sell_decreases_price(self) -> None:
        result = apply_slippage("sell", 100.0, 10.0)
        assert result is not None
        assert result < 100.0

    def test_unknown_side_returns_none(self) -> None:
        result = apply_slippage("short", 100.0, 10.0)
        assert result is None


class TestSettlement:
    def test_base_settlement(self) -> None:
        s = BaseSettlement()
        assert s.settle(1000.0, "USD") == "1000.0 USD"

    def test_t2_settlement(self) -> None:
        s = T2Settlement()
        assert s.settle(1000.0, "USD") == "T+2: 1000.0 USD"

    def test_t2_is_base(self) -> None:
        assert issubclass(T2Settlement, BaseSettlement)
