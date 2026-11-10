"""Reviewed contract for the legacy vendor boundary."""

from typing import TypedDict


class VendorPacket(TypedDict):
    """Vendor fields accepted at the legacy boundary."""

    sensor_id: str
    celsius: float


class Reading(TypedDict):
    """Normalized reading returned to the rest of Signal Box."""

    sensor_id: str
    celsius: float


def decode_packet(raw: VendorPacket) -> Reading:
    """Convert a vendor packet into Signal Box's reviewed shape."""
    return {
        "sensor_id": raw["sensor_id"],
        "celsius": raw["celsius"],
    }
