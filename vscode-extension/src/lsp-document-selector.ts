// Implements [CONFIGEDITOR-SOURCES-OPEN-BUFFER] / [VSIX-LSP-CLIENT-CONFIGURATION].
// Root validation stays in the LSP; patterns merely ensure VS Code synchronizes
// candidate config buffers so the server can own parsing and optimistic locks.

import type { DocumentSelector } from "vscode-languageserver-protocol";

export const BASILISK_DOCUMENT_SELECTOR: DocumentSelector = [
  { scheme: "file", language: "python" },
  { scheme: "file", pattern: "**/pyproject.toml" },
];
