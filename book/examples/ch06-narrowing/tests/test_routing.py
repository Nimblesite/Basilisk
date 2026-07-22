"""Runtime evidence for the Chapter 6 Signal Box checkpoint."""

import unittest

from signal_box.routing import is_reading_event, normalize, route_status


class RoutingTests(unittest.TestCase):
    """Exercise each runtime path independently of the static claims."""

    def test_normalize_routes_all_reading_forms(self) -> None:
        self.assertIsNone(normalize(None))
        self.assertEqual(normalize("21.5"), 21.5)
        self.assertEqual(normalize(18.0), 18.0)

    def test_type_guard_checks_the_boundary_shape(self) -> None:
        self.assertTrue(
            is_reading_event({"sensor_id": "roof-2", "celsius": 21.5})
        )
        self.assertFalse(is_reading_event({"sensor_id": "roof-2"}))

    def test_route_status_handles_the_closed_set(self) -> None:
        self.assertEqual(route_status("ready"), "accept readings")
        self.assertEqual(route_status("offline"), "wait for sensor")
        self.assertEqual(route_status("fault"), "request inspection")


if __name__ == "__main__":
    unittest.main()
