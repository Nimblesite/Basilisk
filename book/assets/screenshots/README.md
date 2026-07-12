# Screenshot workspace

Only real captures from the Basilisk release named by the book belong here.
The figure ledger records the editor/terminal, OS, architecture, theme, zoom,
Python target, Basilisk version, fixture, and capture method.

Use the repository's real capture pipelines where possible. Crop around one
interaction, remove private information before capture, and add callouts in a
separate SVG master. Never generate or manually reconstruct product UI.

Chapter 9 is captured from the book-owned Signal Box workspace with:

```sh
make -C book screenshots
```

The command builds and stages the current Basilisk binaries, launches a headed
VS Code Extension Development Host, waits for the real LSP snapshot and
preview, and writes both 2880 × 1800 PNGs here. The figure ledger records the
environment and keeps the captures behind a versioned-release publication gate.
