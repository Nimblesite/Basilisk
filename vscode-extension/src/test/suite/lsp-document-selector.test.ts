// Implements [CONFIGEDITOR-SOURCES-OPEN-BUFFER].

import * as assert from "assert";

import { BASILISK_DOCUMENT_SELECTOR } from "../../lsp-document-selector";

suite("LSP document selector", () => {
  test("synchronizes Python and both root configuration candidates", () => {
    assert.deepStrictEqual(BASILISK_DOCUMENT_SELECTOR, [
      { scheme: "file", language: "python" },
      { scheme: "file", pattern: "**/pyproject.toml" },
      { scheme: "file", pattern: "**/basilisk.json" },
    ]);
  });
});
