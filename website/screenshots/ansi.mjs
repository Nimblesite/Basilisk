// Implements [WEBSITE-SCREENSHOTS-ANSI]: faithful ANSI SGR → HTML conversion for
// the exact escape sequences `basilisk check --color always` emits. See
// docs/specs/WEBSITE-SCREENSHOTS-SPEC.md.
//
// basilisk uses a small, fixed palette: reset (0), bold (1), and bold foreground
// red (31, errors), yellow (33, warnings), blue (34, gutters), cyan (36, labels).
// We model exactly that set rather than a general 256-colour terminal, so the
// output is deterministic and matches a real macOS Terminal window pixel-for-pixel.

// Colours tuned to match macOS Terminal's default dark profile as it renders the
// real binary — the values our committed reference PNGs were captured with.
const FOREGROUND = {
  default: "#d6d6d6", // unstyled text (the echoed source line)
  bold: "#f4f4f4", // bold, no colour (diagnostic message)
  31: "#ff6b5e", // red — error / summary
  33: "#e8c062", // yellow — warning
  34: "#7d8cff", // blue — `-->`, `|`, `=`, line numbers
  36: "#4ec9d4", // cyan — help / note / see labels
};

const ESCAPE_PATTERN = /\x1b\[([0-9;]*)m/g;

const escapeHtml = (text) =>
  text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

const initialState = () => ({ bold: false, color: null });

// Fold one SGR parameter list into the running style state.
const applyParams = (state, params) => {
  const codes = params === "" ? [0] : params.split(";").map(Number);
  return codes.reduce((next, code) => {
    if (code === 0) return initialState();
    if (code === 1) return { ...next, bold: true };
    if (code >= 30 && code <= 37) return { ...next, color: code };
    if (code >= 90 && code <= 97) return { ...next, color: code - 60 };
    return next;
  }, state);
};

const colorFor = (state) => {
  if (state.color !== null && FOREGROUND[state.color]) return FOREGROUND[state.color];
  return state.bold ? FOREGROUND.bold : FOREGROUND.default;
};

const wrap = (text, state) => {
  if (text === "") return "";
  const weight = state.bold ? "700" : "400";
  return `<span style="color:${colorFor(state)};font-weight:${weight}">${escapeHtml(text)}</span>`;
};

/**
 * Convert a string containing basilisk's ANSI escape sequences into themed HTML.
 * Unstyled runs still emit a span so every glyph carries the terminal foreground.
 */
export const ansiToHtml = (raw) => {
  let html = "";
  let state = initialState();
  let cursor = 0;

  for (const match of raw.matchAll(ESCAPE_PATTERN)) {
    html += wrap(raw.slice(cursor, match.index), state);
    state = applyParams(state, match[1]);
    cursor = match.index + match[0].length;
  }
  html += wrap(raw.slice(cursor), state);
  return html;
};

export const TERMINAL_FOREGROUND = FOREGROUND;
