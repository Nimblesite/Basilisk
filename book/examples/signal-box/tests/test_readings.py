"""Chapter 9 fixture with a test-policy preview target."""

import unittest

from signal_box.readings import normalize_reading


def sample_reading():
    """Return stable example input for the configuration chapter."""
    return dict(sensor_id="north-7", celsius=21.5)


class NormalizeReadingTests(unittest.TestCase):
    """Keep runtime evidence separate from annotation policy."""

    def test_normalizes_sensor_id_and_temperature(self) -> None:
        self.assertEqual(
            normalize_reading(sample_reading()),
            {"sensor_id": "north-7", "celsius": 21.5},
        )
