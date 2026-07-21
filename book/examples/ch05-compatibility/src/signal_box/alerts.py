"""Compatibility examples for the Chapter 5 checkpoint."""

from collections.abc import Callable, Sequence

AlertFormatter = Callable[[float, str], str]


def format_alert(celsius: float, sensor_id: str) -> str:
    """Format one numeric reading for a named sensor."""
    return f"{sensor_id}: {celsius:.1f} °C"


def render_alerts(
    values: Sequence[float],
    sensor_id: str,
    formatter: AlertFormatter = format_alert,
) -> list[str]:
    """Render readings without mutating the caller's collection."""
    return [formatter(value, sensor_id) for value in values]


def append_calibration(values: list[float], calibration: float) -> None:
    """Add a calibration value to a list that permits fractional values."""
    values.append(calibration)
