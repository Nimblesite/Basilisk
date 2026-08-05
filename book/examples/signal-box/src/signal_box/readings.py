"""Small deliberately unannotated boundary used by Chapter 9."""


def normalize_reading(raw):
    """Normalize a raw reading after its boundary policy is chosen."""
    return dict(
        sensor_id=str(raw["sensor_id"]),
        celsius=float(raw["celsius"]),
    )
