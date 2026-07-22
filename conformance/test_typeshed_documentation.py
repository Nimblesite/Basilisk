"""Typeshed documentation integrity gates ([TYPESHEDRT-ACCEPTANCE-GATES])."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PIN = "6ef9f7719ecfff09dad8724ef42b621fd994fb5e"
DOCUMENTS = (
    ROOT / "docs" / "plans" / "CHECKER-TYPESHED-RUNTIME-PLAN.md",
    ROOT / "docs" / "specs" / "CHECKER-STUB-RESOLUTION-SPEC.md",
    ROOT / "docs" / "specs" / "LSP-CONFIGURATION-EDITOR-SPEC.md",
)


class TypeshedDocumentationTests(unittest.TestCase):
    def test_every_typeshed_document_pins_the_typing_authority(self) -> None:
        canonical = f"python/typing/blob/{PIN}/docs/spec/distributing.rst"
        for document in DOCUMENTS:
            text = document.read_text()
            self.assertIn(PIN, text, document)
            self.assertIn(canonical, text, document)

    def test_relative_links_and_explicit_anchors_resolve(self) -> None:
        for document in DOCUMENTS:
            text = document.read_text()
            for target in re.findall(r"]\(([^)]+)\)", text):
                if "://" in target:
                    continue
                if target.startswith("#"):
                    target_path = document
                    anchor = target.removeprefix("#")
                else:
                    path_text, _, anchor = target.partition("#")
                    target_path = (document.parent / path_text).resolve()
                    self.assertTrue(target_path.is_file(), f"{document}: {target}")
                if anchor:
                    target_text = target_path.read_text()
                    self.assertIn(
                        f"{{#{anchor}}}", target_text, f"{document}: {target}"
                    )

    def test_six_step_mermaid_flow_is_retained_in_order(self) -> None:
        spec = DOCUMENTS[1].read_text()
        blocks = re.findall(r"```mermaid\n(.*?)\n```", spec, flags=re.DOTALL)
        self.assertEqual(len(blocks), 1)
        flow = blocks[0]
        self.assertRegex(flow, r"^flowchart (?:TD|LR)\n")
        for step in range(1, 7):
            self.assertIn(f"{step} ·", flow)
        for edge in (
            "S1 -- miss --> S2",
            "S2 -- miss --> S3",
            "S3 -- miss --> S4",
            "S4 -- none --> S5",
            "S5 -- miss --> S6",
            "S6 --> U",
        ):
            self.assertIn(edge, flow)
        self.assertLess(flow.index("1 ·"), flow.index("2 ·"))
        self.assertLess(flow.index("2 ·"), flow.index("3 ·"))
        self.assertLess(flow.index("3 ·"), flow.index("4 ·"))
        self.assertLess(flow.index("4 ·"), flow.index("5 ·"))
        self.assertLess(flow.index("5 ·"), flow.index("6 ·"))

    def test_cache_pin_and_python_target_contract_has_no_contradiction(self) -> None:
        """Store entries are commits, so age and the network carry no meaning.

        The checker never downloads: the only sources are a pinned store entry
        and a custom folder, so no document may describe a moving `latest`
        selection, a refresh TTL, or any expiry window. A store entry is
        content-addressed and re-hashed on every load, so age carries no
        information about it — reuse is unbounded and re-verified, and the pin
        itself never expires or changes. Both the ban on freshness language and
        the positive store contract are asserted here so neither can be
        dropped silently.
        """
        combined = "\n".join(document.read_text() for document in DOCUMENTS)
        normalized = re.sub(r"\s+", " ", combined).lower()
        for forbidden in (
            # Freshness/TTL language: with no downloads there is nothing to
            # refresh, so any expiry window is a contradiction.
            "without a refresh ttl",
            "refresh ttl",
            "24-hour",
            "24 hours",
            "cached indefinitely",
            # A pin must never be presented as expiring: that was the defect
            # this contract originally guarded against.
            "pins expire after 24 hours",
            "an exact pin expires",
            # Commit selection is never derived from the Python target.
            "uses a python-version-to-sha map",
            "selects a commit from python-version",
            "default 3.12",
        ):
            self.assertNotIn(forbidden, normalized)
        # The checker is offline: no download path exists at check time.
        self.assertIn("the checker never downloads", normalized)
        # Store reuse is unbounded, and the reason is stated rather than
        # asserted: every load re-verifies the content address.
        self.assertIn("reused regardless of age", normalized)
        self.assertIn("re-verified by hashing every time", normalized)
        self.assertIn("no expiry, no reuse policy, no cache-off mode", normalized)
        self.assertIn(
            "nothing is cached, nothing expires, a pin always verifies", normalized
        )
        # Identity stability is unchanged by any of the above.
        self.assertIn("the pin never expires or changes", normalized)


if __name__ == "__main__":
    unittest.main()
