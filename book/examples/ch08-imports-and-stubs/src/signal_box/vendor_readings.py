"""Normalize readings supplied by the simulated vendor package."""

from dataclasses import dataclass

from vendor_sensor import fetch_packet


@dataclass(frozen=True)
class Reading:
    """The small domain model Signal Box keeps internally."""

    sensor_id: str
    celsius: float
    state: str


def read_sensor(sensor_id: str) -> Reading | None:
    """Fetch and normalize one vendor packet."""
    packet = fetch_packet(sensor_id, timeout=0.5)
    if packet is None:
        return None
    return Reading(
        sensor_id=packet.sensor_id,
        celsius=packet.celsius,
        state=packet.state,
    )
