// Implements [CONFIGEDITOR-SOURCES-OPEN-BUFFER].

import * as assert from "assert";

import { BASILISK_DOCUMENT_SELECTOR } from "../../lsp-document-selector";

suite("LSP document selector", () => {
  test("synchronizes Python and the pyproject.toml configuration candidate", () => {
    assert.deepStrictEqual(BASILISK_DOCUMENT_SELECTOR, [
      { scheme: "file", language: "python" },
      { scheme: "file", pattern: "**/pyproject.toml" },
    ]);
  });

  test("never synchronizes the removed basilisk.json format", () => {
    const serialized = JSON.stringify(BASILISK_DOCUMENT_SELECTOR);
    assert.ok(
      !serialized.includes("basilisk.json"),
      `selector must not reference basilisk.json: ${serialized}`,
    );
  });
});
