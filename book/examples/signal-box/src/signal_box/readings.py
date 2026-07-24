"""Small deliberately incomplete boundary used by Chapter 9."""


def normalize_reading(raw) -> None:
    """Normalize a raw reading after its boundary policy is chosen."""
    return {
        "sensor_id": str(raw["sensor_id"]),
        "celsius": float(raw["celsius"]),
    }
