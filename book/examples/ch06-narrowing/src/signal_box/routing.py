"""Control-flow narrowing examples for the Chapter 6 checkpoint."""

from typing import Literal, TypeGuard, TypedDict, assert_never


class ReadingEvent(TypedDict):
    """Validated dictionary-shaped input from a sensor boundary."""

    sensor_id: str
    celsius: float


Status = Literal["ready", "offline", "fault"]


def normalize(value: float | str | None) -> float | None:
    """Normalize an absent, numeric, or textual reading."""
    if value is None:
        return None
    if isinstance(value, str):
        return float(value)
    return value


def is_reading_event(value: object) -> TypeGuard[ReadingEvent]:
    """Return whether an unknown runtime value has the required shape."""
    return (
        isinstance(value, dict)
        and isinstance(value.get("sensor_id"), str)
        and isinstance(value.get("celsius"), float)
    )


def route_status(status: Status) -> str:
    """Return one message for every status in the closed set."""
    match status:
        case "ready":
            return "accept readings"
        case "offline":
            return "wait for sensor"
        case "fault":
            return "request inspection"
        case unreachable:
            assert_never(unreachable)
