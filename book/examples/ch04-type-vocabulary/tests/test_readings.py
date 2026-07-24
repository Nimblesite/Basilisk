"""Runtime evidence for the Chapter 4 Signal Box checkpoint."""

import unittest

from signal_box.readings import normalize_reading, normalize_temperature


class ReadingTests(unittest.TestCase):
    """Keep normalization behavior separate from annotation claims."""

    def test_normalize_temperature_preserves_absence(self) -> None:
        self.assertIsNone(normalize_temperature(None))

    def test_normalize_temperature_converts_text(self) -> None:
        self.assertEqual(normalize_temperature("21.5"), 21.5)

    def test_normalize_reading_converts_present_fields(self) -> None:
        self.assertEqual(
            normalize_reading({"sensor_id": "north-7", "celsius": "21.5"}),
            {"sensor_id": "north-7", "celsius": 21.5},
        )


if __name__ == "__main__":
    unittest.main()
