// Implements [VSIX-CONFIGURATION-EDITOR-HOST] / [CONFIGEDITOR-ACCESSIBILITY-SECURITY].
/** Static, data-free configuration editor document assembled by the hardened host. */

import { buildWebviewDocument } from "./profiler-webview";
import { CONFIGURATION_EDITOR_SCRIPT } from "./configuration-editor-script";
import { CONFIGURATION_EDITOR_STYLES } from "./configuration-editor-styles";

// [CONFIGEDITOR-VSIX-EXPERIENCE]: one tag-first Rules view. Tag groups carry
// the tag-entry control; rows carry per-rule entry controls. There is no
// adoption view, no path overrides, and no preset UI.
const RULES_SECTION = `
  <section id="rules-section" aria-labelledby="rules-title">
    <div id="rules-layout">
      <aside id="tag-rail" aria-labelledby="tags-title"><h2 id="tags-title">Tags</h2><div id="tag-list"></div></aside>
      <div id="rules-workspace">
        <div id="rules-toolbar"><h2 id="rules-title" class="sr-only">Rules</h2><label class="sr-only" for="rule-search">Search rules</label><input id="rule-search" type="search" autocomplete="off" spellcheck="false" placeholder="Search rules · tag:pep · severity:error · has:diagnostics"><span id="filter-result" aria-live="polite"></span></div>
        <div id="rule-viewport" tabindex="0" role="region" aria-label="Configuration rules" aria-keyshortcuts="ArrowUp ArrowDown PageUp PageDown Home End"><div id="rule-spacer" role="list"><div id="rule-window"></div></div></div>
      </div>
      <aside id="rule-detail" aria-labelledby="detail-title"><h2 id="detail-title">Rule detail</h2><div id="detail-empty">Select a rule to see its entry, effective severity, and workspace occurrences.</div><div id="detail-content" hidden></div></aside>
    </div>
  </section>`;

const OVERLAYS = `
  <dialog id="preview-dialog" aria-labelledby="preview-title"><header><h2 id="preview-title">Review exact impact</h2></header><div id="preview-body"><div id="impact-grid"></div><h3>Exact resolved changes</h3><div id="preview-changes"></div></div><footer><button type="button" class="secondary" data-action="close-preview">Cancel</button><button type="button" class="primary" data-action="apply-preview">Apply change</button></footer></dialog>
  <div id="state-overlay" role="dialog" aria-modal="true" aria-labelledby="state-title" aria-describedby="state-message" tabindex="-1" hidden><div id="state-card" class="card"><div id="state-symbol" aria-hidden="true">B</div><h2 id="state-title">Loading configuration</h2><p id="state-message"></p><div class="action-row overlay-actions"><button id="state-open-raw" type="button" class="secondary" data-action="open-raw" hidden>Open raw configuration</button><button id="state-action" type="button" class="primary" data-action="refresh" hidden>Try again</button></div></div></div>
  <div id="announcer" class="sr-only" aria-live="polite" aria-atomic="true"></div>`;

const BODY = `
  <a id="skip-link" href="#configuration-main">Skip to configuration</a>
  <header><div id="identity"><div id="mark" aria-hidden="true">B</div><div><h1>Basilisk Configuration</h1><div id="root-label">Waiting for workspace…</div></div></div><div id="source-block"><span id="source-label">No active source</span></div><div id="header-actions"><span id="status-pill" data-phase="idle">Connecting…</span><button type="button" class="icon-button" data-action="refresh" aria-label="Refresh configuration">↻</button><button type="button" class="secondary" data-action="open-raw">Open raw</button></div></header>
  <div id="shell"><main id="configuration-main">${RULES_SECTION}</main></div>
  ${OVERLAYS}`;

/** Build the complete CSP-locked document; no workspace data is interpolated. */
export function buildConfigurationEditorDocument(): string {
  return buildWebviewDocument({
    title: "Basilisk Configuration",
    css: CONFIGURATION_EDITOR_STYLES,
    body: BODY,
    script: CONFIGURATION_EDITOR_SCRIPT,
  });
}
