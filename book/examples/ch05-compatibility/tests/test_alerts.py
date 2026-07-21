"""Runtime evidence for the Chapter 5 Signal Box checkpoint."""

import unittest

from signal_box.alerts import append_calibration, format_alert, render_alerts


class AlertTests(unittest.TestCase):
    """Keep runtime behavior separate from compatibility claims."""

    def test_format_alert_accepts_integer_value(self) -> None:
        self.assertEqual(format_alert(21, "roof-2"), "roof-2: 21.0 °C")

    def test_render_alerts_reads_from_a_tuple(self) -> None:
        self.assertEqual(
            render_alerts((18, 18.5), "north-7"),
            ["north-7: 18.0 °C", "north-7: 18.5 °C"],
        )

    def test_render_alerts_uses_a_compatible_callback(self) -> None:
        def compact(celsius: float, sensor_id: str) -> str:
            return f"{sensor_id}={celsius:g}"

        self.assertEqual(
            render_alerts([21.5], "roof-2", compact),
            ["roof-2=21.5"],
        )

    def test_append_calibration_accepts_fractional_value(self) -> None:
        readings: list[float] = [18, 19]
        append_calibration(readings, 18.5)
        self.assertEqual(readings, [18, 19, 18.5])


if __name__ == "__main__":
    unittest.main()
