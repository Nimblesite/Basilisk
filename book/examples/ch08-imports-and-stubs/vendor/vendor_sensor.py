"""Small untyped stand-in for a third-party sensor package."""


class Packet:
    """One packet returned by the simulated vendor API."""

    def __init__(self, sensor_id, celsius, state):
        self.sensor_id = sensor_id
        self.celsius = celsius
        self.state = state


def fetch_packet(sensor_id, *, timeout=1.0):
    """Return one packet, or None when the sensor is offline."""
    if timeout <= 0:
        raise ValueError("timeout must be positive")
    if sensor_id == "offline":
        return None
    state = "warning" if sensor_id.startswith("hot-") else "normal"
    celsius = 38.5 if state == "warning" else 21.5
    return Packet(sensor_id, celsius, state)
