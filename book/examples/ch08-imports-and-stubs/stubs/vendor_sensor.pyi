from typing import Literal

class Packet:
    sensor_id: str
    celsius: float
    state: Literal["normal", "warning"]

def fetch_packet(
    sensor_id: str,
    *,
    timeout: float = ...,
) -> Packet | None: ...
