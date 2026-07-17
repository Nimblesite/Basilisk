"""Everyday type vocabulary for the Chapter 4 checkpoint."""

ReadingValue = str | float | None
RawReading = dict[str, ReadingValue]
NormalizedReading = dict[str, ReadingValue]


def normalize_temperature(value: ReadingValue) -> float | None:
    """Convert one present temperature to a float."""
    if value is None:
        return None
    return float(value)


def normalize_reading(raw: RawReading) -> NormalizedReading:
    """Normalize the fields used by the current Signal Box checkpoint."""
    raw_sensor_id = raw.get("sensor_id")
    sensor_id = "unknown" if raw_sensor_id is None else str(raw_sensor_id)
    celsius = normalize_temperature(raw.get("celsius"))
    return {"sensor_id": sensor_id, "celsius": celsius}
