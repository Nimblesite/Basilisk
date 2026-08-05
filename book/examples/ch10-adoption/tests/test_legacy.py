"""Runtime evidence for the Chapter 10 adoption checkpoint."""

import unittest

from signal_box.legacy.decoder import decode_packet
from signal_box.legacy.status import FALLBACK_LABEL, status_label


class LegacyBoundaryTests(unittest.TestCase):
    """Keep behavior checked while static debt is paid down."""

    def test_decodes_a_vendor_packet(self) -> None:
        self.assertEqual(
            decode_packet({"sensor_id": "north-7", "celsius": 21.5}),
            {"sensor_id": "north-7", "celsius": 21.5},
        )

    def test_preserves_the_runtime_fallback(self) -> None:
        self.assertEqual(FALLBACK_LABEL, "offline")
        self.assertEqual(status_label(7), "7")
