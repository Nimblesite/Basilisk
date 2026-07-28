"""Reusable data and behavior contracts for the Chapter 7 checkpoint."""

from dataclasses import dataclass
from enum import Enum
from typing import Generic, NotRequired, Protocol, TypeVar, TypedDict, overload


class RawReading(TypedDict):
    """A validated, dictionary-shaped reading at the input boundary."""

    sensor_id: str
    celsius: float
    state: NotRequired[str]


class AlertState(Enum):
    """The finite states owned by the Signal Box domain."""

    NORMAL = "normal"
    WARNING = "warning"
    OFFLINE = "offline"


@dataclass(frozen=True)
class Reading:
    """An attribute-shaped reading used inside the application."""

    sensor_id: str
    celsius: float
    state: AlertState


class ReadingStore(Protocol):
    """The storage operations consumed by the reporting workflow."""

    def save(self, reading: Reading) -> None:
        """Store one reading."""
        ...

    def readings(self) -> tuple[Reading, ...]:
        """Return the readings currently exposed by this store."""
        ...


class MemoryStore:
    """Retain every reading in insertion order."""

    def __init__(self) -> None:
        self._readings: list[Reading] = []

    def save(self, reading: Reading) -> None:
        """Append one reading."""
        self._readings.append(reading)

    def readings(self) -> tuple[Reading, ...]:
        """Return an immutable snapshot of all readings."""
        return tuple(self._readings)


class LatestBySensorStore:
    """Retain only the latest reading for each sensor identifier."""

    def __init__(self) -> None:
        self._readings: dict[str, Reading] = {}

    def save(self, reading: Reading) -> None:
        """Replace the reading stored for the same sensor."""
        self._readings[reading.sensor_id] = reading

    def readings(self) -> tuple[Reading, ...]:
        """Return an immutable snapshot of the latest readings."""
        return tuple(self._readings.values())


Payload = TypeVar("Payload")


@dataclass(frozen=True)
class Page(Generic[Payload]):
    """A report page that preserves the type of its items."""

    items: tuple[Payload, ...]
    total: int


def validate_raw(value: object) -> RawReading:
    """Validate an unknown runtime value and return its typed shape."""
    if not isinstance(value, dict):
        raise ValueError("reading must be a dictionary")

    sensor_id = value.get("sensor_id")
    celsius = value.get("celsius")
    state = value.get("state", "normal")
    if not isinstance(sensor_id, str):
        raise ValueError("sensor_id must be a string")
    if not isinstance(celsius, (int, float)) or isinstance(celsius, bool):
        raise ValueError("celsius must be numeric")
    if not isinstance(state, str):
        raise ValueError("state must be a string")

    raw: RawReading = {"sensor_id": sensor_id, "celsius": float(celsius)}
    if "state" in value:
        raw["state"] = state
    return raw


def reading_from_raw(raw: RawReading) -> Reading:
    """Transform validated boundary data into the domain model."""
    return Reading(
        sensor_id=raw["sensor_id"],
        celsius=raw["celsius"],
        state=AlertState(raw.get("state", "normal")),
    )


@overload
def ingest(payload: RawReading) -> Reading: ...


@overload
def ingest(payload: list[RawReading]) -> list[Reading]: ...


def ingest(payload: RawReading | list[RawReading]) -> Reading | list[Reading]:
    """Transform either one validated reading or a batch of readings."""
    if isinstance(payload, list):
        return [reading_from_raw(item) for item in payload]
    return reading_from_raw(payload)


def page_readings(store: ReadingStore, limit: int) -> Page[Reading]:
    """Read a bounded page through the consumed storage protocol."""
    if limit < 0:
        raise ValueError("limit must not be negative")
    readings = store.readings()
    return Page(items=readings[:limit], total=len(readings))
