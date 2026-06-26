// Implements [WEBSITE-SCREENSHOTS-CHROME]: the macOS Terminal.app window chrome
// (traffic-light buttons, folder + title bar, dark body) the CLI screenshots are
// framed in. See docs/specs/WEBSITE-SCREENSHOTS-SPEC.md.
//
// This reproduces in HTML what the old manual process captured with
// Terminal.app + screencapture: a 120-column window titled "basilisk-demo — -zsh"
// on the default dark profile, so regenerated images are visually identical to
// the originals but fully reproducible and PII-free.

import { ansiToHtml } from "./ansi.mjs";

// 120-column window, matching the original `120×26` captures. Width is fixed in
// `ch` so every screenshot lines up at the same column width; height is content.
const COLUMNS = 120;
const TITLE = "basilisk-demo — -zsh";

// macOS "open folder" Finder glyph, inlined so rendering needs no network/font.
const FOLDER_ICON = `<svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
  <path d="M1.5 4.2c0-.6.5-1.1 1.1-1.1h3.1c.3 0 .6.1.8.4l.8.9h5.1c.6 0 1.1.5 1.1 1.1v1H1.5V4.2z" fill="#7fb3ff"/>
  <path d="M1.5 6.1h13.1l-1 6c-.1.5-.5.9-1.1.9H2.6c-.5 0-1-.4-1.1-.9l-1-6z" fill="#9cc6ff"/>
</svg>`;

const STYLE = `
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body { background: transparent; }
  body { padding: 24px; display: inline-block; }
  .window {
    display: inline-block;
    border-radius: 10px;
    overflow: hidden;
    box-shadow: 0 22px 70px rgba(0, 0, 0, 0.55);
    font-family: "SF Mono", "Menlo", "Monaco", "Consolas", monospace;
  }
  .titlebar {
    position: relative;
    height: 30px;
    display: flex;
    align-items: center;
    padding: 0 12px;
    background: linear-gradient(#3c3c3e, #303032);
    border-bottom: 1px solid #1f1f21;
  }
  .lights { display: flex; gap: 8px; }
  .light { width: 13px; height: 13px; border-radius: 50%; }
  .light.close  { background: #ff5f57; border: 0.5px solid #e0443e; }
  .light.min    { background: #febc2e; border: 0.5px solid #dea123; }
  .light.expand { background: #28c840; border: 0.5px solid #1aab29; }
  .title {
    position: absolute;
    left: 0; right: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font: 500 13px -apple-system, "SF Pro Text", "Helvetica Neue", sans-serif;
    color: #c7c7c9;
    pointer-events: none;
  }
  .body {
    background: rgb(30, 30, 30);
    color: #d6d6d6;
    padding: 14px 18px 16px;
    font-size: 12px;
    line-height: 1.5;
    width: ${COLUMNS}ch;
  }
  .body pre {
    font-family: inherit;
    font-size: inherit;
    white-space: pre-wrap;
    word-break: break-word;
    tab-size: 4;
  }
  .prompt { color: #d6d6d6; }
`;

// One Terminal "size" suffix per window, e.g. "120×26", computed from the lines
// shown so the title bar reads like a real session.
const sizeSuffix = (lineCount) => `${COLUMNS}×${Math.max(lineCount, 26)}`;

/**
 * Build a complete HTML document for one screenshot.
 *
 * @param {string} command - the command echoed after the prompt, e.g. "basilisk check e0001.py".
 * @param {string} ansiOutput - raw stdout from the binary, including ANSI escapes.
 */
export const buildTerminalHtml = ({ command, ansiOutput }) => {
  const outputHtml = ansiToHtml(ansiOutput.replace(/\n+$/, ""));
  const lineCount = ansiOutput.split("\n").length + 3;
  const body = `<span class="prompt">$ </span>${command}\n${outputHtml}\n<span class="prompt">$ </span>`;

  return `<!doctype html>
<html><head><meta charset="utf-8"><style>${STYLE}</style></head>
<body>
  <div class="window">
    <div class="titlebar">
      <div class="lights">
        <span class="light close"></span>
        <span class="light min"></span>
        <span class="light expand"></span>
      </div>
      <div class="title">${FOLDER_ICON}<span>${TITLE} — ${sizeSuffix(lineCount)}</span></div>
    </div>
    <div class="body"><pre>${body}</pre></div>
  </div>
</body></html>`;
};

export const WINDOW_SELECTOR = ".window";
