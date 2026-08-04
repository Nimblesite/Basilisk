"""Runtime evidence for the Chapter 8 checkpoint."""

import unittest

from signal_box.vendor_readings import Reading, read_sensor
from vendor_sensor import fetch_packet


class VendorReadingTests(unittest.TestCase):
    """Exercise the runtime behavior promised by the reviewed stub."""

    def test_normal_packet_becomes_reading(self) -> None:
        self.assertEqual(
            read_sensor("roof-2"),
            Reading(sensor_id="roof-2", celsius=21.5, state="normal"),
        )

    def test_hot_packet_keeps_warning_state(self) -> None:
        self.assertEqual(
            read_sensor("hot-yard-1"),
            Reading(sensor_id="hot-yard-1", celsius=38.5, state="warning"),
        )

    def test_offline_sensor_returns_none(self) -> None:
        self.assertIsNone(read_sensor("offline"))

    def test_timeout_is_keyword_only_at_runtime(self) -> None:
        with self.assertRaises(TypeError):
            fetch_packet("roof-2", 0.5)

        with self.assertRaises(ValueError):
            fetch_packet("roof-2", timeout=0.0)


if __name__ == "__main__":
    unittest.main()
