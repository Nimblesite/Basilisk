"""Runtime evidence for the Chapter 7 Signal Box checkpoint."""

import unittest

from signal_box.contracts import (
    AlertState,
    LatestBySensorStore,
    MemoryStore,
    Reading,
    ReadingStore,
    ingest,
    page_readings,
    reading_from_raw,
    validate_raw,
)


class ContractTests(unittest.TestCase):
    """Exercise the runtime work behind each static contract."""

    def test_validate_raw_checks_required_and_optional_fields(self) -> None:
        raw = validate_raw({"sensor_id": "roof-2", "celsius": 21, "state": "warning"})
        self.assertEqual(raw["celsius"], 21.0)
        self.assertEqual(raw["state"], "warning")

        with self.assertRaises(ValueError):
            validate_raw({"sensor_id": "roof-2"})
        with self.assertRaises(ValueError):
            validate_raw({"sensor_id": "roof-2", "celsius": True})

    def test_reading_from_raw_builds_the_domain_model(self) -> None:
        reading = reading_from_raw({"sensor_id": "roof-2", "celsius": 21.5})
        self.assertEqual(reading.sensor_id, "roof-2")
        self.assertEqual(reading.state, AlertState.NORMAL)

        with self.assertRaises(ValueError):
            reading_from_raw(
                {"sensor_id": "roof-2", "celsius": 21.5, "state": "unknown"}
            )

    def test_ingest_preserves_single_and_batch_result_shapes(self) -> None:
        raw = {"sensor_id": "roof-2", "celsius": 21.5}
        one = ingest(raw)
        batch = ingest([raw])
        self.assertIsInstance(one, Reading)
        self.assertEqual(batch, [one])

    def test_both_stores_satisfy_the_consumed_workflow(self) -> None:
        for store in (MemoryStore(), LatestBySensorStore()):
            self._exercise_store(store)

    def _exercise_store(self, store: ReadingStore) -> None:
        first = Reading("roof-2", 21.5, AlertState.NORMAL)
        second = Reading("yard-1", 17.0, AlertState.WARNING)
        store.save(first)
        store.save(second)

        page = page_readings(store, limit=1)
        self.assertEqual(page.items, (first,))
        self.assertEqual(page.total, 2)


if __name__ == "__main__":
    unittest.main()
