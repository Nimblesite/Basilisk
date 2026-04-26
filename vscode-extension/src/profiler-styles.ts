/**
 * Shared CSS and utility functions for Basilisk profiler webviews.
 *
 * Centralises the brand palette, typography, and common UI component
 * styles used by the flamegraph, reference graph, and memory dashboard.
 */

// ── Brand palette CSS custom properties ──────────────────────────────────

export const PROFILER_CSS_VARS = `
    :root {
      --prof-critical: #e8500a;
      --prof-hot: #f97316;
      --prof-warm: #fbbf24;
      --prof-cool: #4a5468;
      --prof-idle: #1a1f2e;
      --prof-mem-critical: #c084fc;
      --prof-mem-hot: #a78bfa;
      --prof-mem-leak: #f87171;
      --prof-success: #34d399;
      --prof-info: #60a5fa;
      --prof-bg: #0a0c12;
      --prof-surface: #141820;
      --prof-border: #1a1f2e;
      --prof-text: #f0f2f7;
      --prof-text-secondary: #8892a4;
    }`;

// ── Base reset and body styles ───────────────────────────────────────────

export const PROFILER_CSS_RESET = `
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      background: var(--prof-bg);
      color: var(--prof-text);
      font-family: 'Space Grotesk', -apple-system, sans-serif;
    }`;

// ── Shared component styles (cards, tables, bars) ────────────────────────

export const PROFILER_CSS_CARDS = `
    .summary-cards {
      display: flex;
      gap: 12px;
      margin-bottom: 20px;
      flex-wrap: wrap;
    }
    .card {
      background: var(--prof-surface);
      border: 1px solid var(--prof-border);
      border-radius: 8px;
      padding: 12px 16px;
      min-width: 120px;
    }
    .card .label {
      font-size: 11px;
      color: var(--prof-text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
    .card .value {
      font-size: 24px;
      font-weight: 600;
      font-family: 'JetBrains Mono', monospace;
      margin-top: 4px;
    }`;

export const PROFILER_CSS_TABLE = `
    .data-table {
      width: 100%;
      border-collapse: collapse;
    }
    .data-table th {
      text-align: left;
      font-size: 11px;
      color: var(--prof-text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      padding: 6px 8px;
      border-bottom: 1px solid var(--prof-border);
    }
    .data-table td {
      padding: 8px;
      font-family: 'JetBrains Mono', monospace;
      font-size: 13px;
      border-bottom: 1px solid var(--prof-border);
      cursor: pointer;
    }
    .data-table tr:hover td { background: var(--prof-surface); }
    .bar-cell { position: relative; width: 160px; }
    .bar {
      height: 20px;
      border-radius: 3px;
      transition: width 200ms ease;
    }`;

export const PROFILER_CSS_HEADING = `
    h1 {
      font-size: 18px;
      font-weight: 600;
      margin-bottom: 16px;
      display: flex;
      align-items: center;
      gap: 8px;
    }
    h2 {
      font-size: 14px;
      font-weight: 600;
      color: var(--prof-text-secondary);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      margin: 16px 0 8px;
    }`;

// ── Byte size constants ──────────────────────────────────────────────────

export const BYTES_PER_KB = 1024;
export const BYTES_PER_MB = 1_048_576;
export const BYTES_PER_GB = 1_073_741_824;

/** Format a byte count as a human-readable string (e.g. "24.5 MB"). */
export function formatBytes(bytes: number): string {
  if (bytes >= BYTES_PER_GB) { return `${(bytes / BYTES_PER_GB).toFixed(1)} GB`; }
  if (bytes >= BYTES_PER_MB) { return `${(bytes / BYTES_PER_MB).toFixed(1)} MB`; }
  if (bytes >= BYTES_PER_KB) { return `${(bytes / BYTES_PER_KB).toFixed(1)} KB`; }
  return `${bytes} B`;
}

// ── Shared inline JS utilities ───────────────────────────────────────────

export const PROFILER_JS_UTILS = `
    function formatBytes(bytes) {
      if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB';
      if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + ' MB';
      if (bytes >= 1024) return (bytes / 1024).toFixed(1) + ' KB';
      return bytes + ' B';
    }
    function basename(filePath) {
      return filePath.split(/[\\/\\\\]/).pop() || filePath;
    }
    function escapeHtml(str) {
      return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }`;
