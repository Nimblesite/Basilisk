import unittest

from stability import DEFAULT_MAX_CV, coefficient_of_variation, measurement_is_noisy


class StabilityTests(unittest.TestCase):
    def test_default_cutoff_catches_observed_moderate_scheduler_noise(self) -> None:
        self.assertEqual(DEFAULT_MAX_CV, 0.15)
        result = {"mean": 0.0098, "stddev": 0.0017}

        self.assertTrue(measurement_is_noisy(result, max_cv=DEFAULT_MAX_CV))

    def test_large_scheduler_outliers_require_a_longer_measurement(self) -> None:
        result = {"mean": 0.0120, "stddev": 0.0053}

        self.assertTrue(measurement_is_noisy(result, max_cv=DEFAULT_MAX_CV))

    def test_normal_process_variance_does_not_trigger_a_retry(self) -> None:
        result = {"mean": 0.0090, "stddev": 0.0012}

        self.assertFalse(measurement_is_noisy(result, max_cv=DEFAULT_MAX_CV))

    def test_zero_mean_is_rejected_as_unstable(self) -> None:
        result = {"mean": 0.0, "stddev": 0.0}

        self.assertEqual(coefficient_of_variation(result), float("inf"))
        self.assertTrue(measurement_is_noisy(result, max_cv=DEFAULT_MAX_CV))


if __name__ == "__main__":
    unittest.main()
