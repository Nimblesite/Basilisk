# Screenshot workspace

Only direct captures from the Basilisk release named by the book belong here.
A mock, reconstruction, generated image, hand-drawn interface, or UI-shaped
diagram is forbidden. Renaming one of those things as a “map,” “wireframe,” or
“diagram” does not make it product evidence.

The figure ledger records the editor/terminal, OS, architecture, theme,
viewport, Basilisk version, fixture, capture method, untouched master SHA-256,
and verified release-artifact SHA-256.

Use the repository's real capture pipelines where possible. Crop around one
interaction, remove private information before capture, and add callouts in a
separate SVG master. Never generate or manually reconstruct product UI.

Chapter 9 is captured from the book-owned Signal Box workspace with:

```sh
make -C book screenshots
```

The command reads the pinned tag and official VSIX checksum from `book.json`,
downloads that release's source tag and published VSIX, rejects any checksum or
version mismatch, and drives the shipped extension code and binaries inside an
isolated headed VS Code Extension Development Host. It never builds or stages
the current checkout as release evidence and never touches an existing VS Code
profile or process.

The capture waits for the real LSP snapshot and preview. It preserves both
2880 × 1800 full-window captures under `masters/`, then makes deterministic
1600 × 1000 publication crops here so interface text remains readable in the
EPUB. Cropping and uniform resizing are the only product-pixel transformations;
the capture is never repainted or composited. The figure ledger records the
environment and keeps every capture behind the versioned-release publication
gate.
