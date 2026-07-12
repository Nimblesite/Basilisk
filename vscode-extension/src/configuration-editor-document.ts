// Implements [VSIX-CONFIGURATION-EDITOR-HOST] / [CONFIGEDITOR-ACCESSIBILITY-SECURITY].
/** Static, data-free configuration editor document assembled by the hardened host. */

import { buildWebviewDocument } from "./profiler-webview";
import { CONFIGURATION_EDITOR_SCRIPT } from "./configuration-editor-script";
import { CONFIGURATION_EDITOR_STYLES } from "./configuration-editor-styles";

const SECTION_NAV = `
  <nav id="section-nav" aria-label="Configuration sections">
    <button type="button" data-section-target="overview" aria-current="page"><span aria-hidden="true">⌂</span><span>Overview</span></button>
    <button type="button" data-section-target="rules"><span aria-hidden="true">☷</span><span>Rules</span></button>
    <button type="button" data-section-target="adoption"><span aria-hidden="true">↗</span><span>Adoption</span></button>
    <button type="button" data-section-target="paths"><span aria-hidden="true">⑂</span><span>Path Overrides</span></button>
    <button type="button" data-section-target="project"><span aria-hidden="true">⚙</span><span>Project</span></button>
  </nav>`;

const OVERVIEW_SECTION = `
  <section id="overview-section" data-section="overview" aria-labelledby="overview-title">
    <div class="section-heading"><h2 id="overview-title">Project policy at a glance</h2><p>Exact effective state from the Basilisk language server—never a synthetic score.</p></div>
    <div id="problem-banner" role="alert" hidden></div>
    <div id="overview-grid">
      <div id="severity-strip" class="card" aria-label="Rule severity totals"></div>
      <article class="card" data-accent="orange"><h3>Current debt</h3><strong class="stat" id="debt-diagnostics">—</strong><p>remaining diagnostics across this workspace</p><button type="button" class="secondary" data-action="show-current">Review occurrences</button></article>
      <article class="card" data-accent="sky"><h3>Adoption exceptions</h3><strong class="stat" id="debt-adoptions">—</strong><p>explicit exceptions kept visible while debt is paid down</p><button type="button" class="secondary" data-section-target="adoption">Open adoption view</button></article>
      <article class="card"><h3>Suppressions</h3><strong class="stat" id="debt-suppressions">—</strong><p>auditable source directives reported by the checker</p><button type="button" class="secondary" data-action="show-suppressions">Inspect suppressions</button></article>
      <article class="card wide" data-accent="orange"><h3>Set the target deliberately</h3><p>Native enables every rule at its own severity. Maximum promotes every rule to an error. Disable all is a broad project policy and always previews its effect first.</p><div class="action-row"><button type="button" class="primary" data-action="native-all">Enable all · native</button><button type="button" class="secondary" data-action="maximum-all">Maximum policy</button><button type="button" class="secondary" data-action="disable-all">Disable all…</button></div></article>
    </div>
  </section>`;

const RULES_SECTION = `
  <section id="rules-section" data-section="rules" aria-labelledby="rules-title" hidden>
    <div id="rules-layout">
      <aside id="tag-rail" aria-labelledby="tags-title"><h2 id="tags-title">Tags</h2><div id="tag-list"></div></aside>
      <div id="rules-workspace">
        <div id="rules-toolbar"><h2 id="rules-title" class="sr-only">Rules</h2><label class="sr-only" for="rule-search">Search rules</label><input id="rule-search" type="search" autocomplete="off" spellcheck="false" placeholder="Search rules · tag:strictness · fix:none"><span id="filter-result" aria-live="polite"></span></div>
        <div id="rule-viewport" tabindex="0" role="region" aria-label="Configuration rules"><div id="rule-spacer"><div id="rule-window"></div></div></div>
      </div>
      <aside id="rule-detail" aria-labelledby="detail-title"><h2 id="detail-title">Rule detail</h2><div id="detail-empty">Select a rule to see its source, impact, and workspace occurrences.</div><div id="detail-content" hidden></div></aside>
    </div>
  </section>`;

const ADOPTION_SECTION = `
  <section id="adoption-section" data-section="adoption" aria-labelledby="adoption-title" hidden>
    <div class="section-heading"><h2 id="adoption-title">Strict-first adoption</h2><p>Set the strongest useful target, take every safe fix, then record only the debt that remains.</p></div>
    <div id="adoption-grid">
      <article class="card" data-accent="orange"><h3>1 · Set the target</h3><p>Enable the complete catalog at native severity without inventing a hidden mode.</p><button type="button" class="primary" data-action="native-all">Preview native policy</button></article>
      <article class="card" data-accent="sky"><h3>2 · Take the free wins</h3><p>Calculate safe fixes together with the target policy before anything is written.</p><button type="button" class="primary" data-action="adopt-safe">Preview with safe fixes</button></article>
      <article class="card"><h3>3 · Review remaining debt</h3><p>Find occurrences with no safe fix. Nothing is silently disabled.</p><button type="button" class="secondary" data-action="show-unfixable">Review unfixable debt</button></article>
      <article class="card wide"><h3>Choose exceptions at the narrowest scope</h3><p>Use Rules for project policy or Path Overrides for a bounded subtree. Per-file adoption stays server-owned so new files remain strict.</p><div class="action-row"><button type="button" class="secondary" data-section-target="rules">Configure rules</button><button type="button" class="secondary" data-section-target="paths">Add path override</button></div></article>
    </div>
  </section>`;

const PATHS_SECTION = `
  <section id="paths-section" data-section="paths" aria-labelledby="paths-title" hidden>
    <div class="section-heading"><h2 id="paths-title">Path overrides</h2><p>Apply the currently selected rules or tag to one explicit project-relative pattern.</p></div>
    <div id="project-grid">
      <form id="path-form" class="card wide">
        <h3>Preview a bounded exception</h3><p>The server validates the pattern, expands the selector, and shows the exact impact.</p>
        <label for="path-pattern">Project-relative path pattern</label><input id="path-pattern" type="text" placeholder="legacy/**" autocomplete="off">
        <label for="path-setting">Severity</label><select id="path-setting"><option value="Warning">Warning</option><option value="Info">Info</option><option value="Disabled">Disabled</option><option value="Inherit">Inherited · reset</option></select>
        <div class="action-row"><button type="submit" class="primary">Preview path override</button><button type="button" class="secondary" data-section-target="rules">Select rules</button></div>
      </form>
    </div>
  </section>`;

const PROJECT_SECTION = `
  <section id="project-section" data-section="project" aria-labelledby="project-title" hidden>
    <div class="section-heading"><h2 id="project-title">Project source</h2><p>The active source and all shadowed configuration are resolved by the language server.</p></div>
    <div id="project-grid">
      <article class="card wide" data-accent="sky"><h3>Active configuration</h3><dl id="source-details"></dl><div class="action-row"><button type="button" class="primary" data-action="open-raw">Open raw configuration</button><button type="button" class="secondary" data-action="refresh">Refresh</button></div></article>
      <article class="card wide"><h3>Configuration problems</h3><div id="problem-list" class="empty-state">No configuration problems.</div></article>
    </div>
  </section>`;

const OVERLAYS = `
  <div id="bulk-tray" role="region" aria-label="Bulk rule actions" hidden><span id="selection-count">0 selected</span><label class="sr-only" for="bulk-setting">Bulk severity</label><select id="bulk-setting"><option value="Native">Native severity</option><option value="Error">Error</option><option value="Warning">Warning</option><option value="Info">Info</option><option value="Disabled">Disabled</option><option value="Inherit">Inherited · reset</option></select><button type="button" class="primary" data-action="preview-bulk">Preview changes</button><button type="button" class="secondary" data-action="clear-selection">Clear</button></div>
  <dialog id="preview-dialog" aria-labelledby="preview-title"><header><h2 id="preview-title">Review exact impact</h2></header><div id="preview-body"><div id="impact-grid"></div><p id="expanded-rules"></p><div id="preview-problems"></div></div><footer><button type="button" class="secondary" data-action="close-preview">Keep editing</button><button type="button" class="primary" data-action="apply-preview">Apply once</button></footer></dialog>
  <div id="state-overlay" hidden><div id="state-card" class="card" role="status"><div id="state-symbol" aria-hidden="true">B</div><h2 id="state-title">Loading configuration</h2><p id="state-message"></p><button id="state-action" type="button" class="primary" data-action="refresh" hidden>Try again</button></div></div>
  <div id="announcer" class="sr-only" aria-live="polite" aria-atomic="true"></div>`;

const BODY = `
  <a id="skip-link" href="#configuration-main">Skip to configuration</a>
  <header><div id="identity"><div id="mark" aria-hidden="true">B</div><div><h1>Basilisk Configuration</h1><div id="root-label">Waiting for workspace…</div></div></div><div id="source-block"><span id="source-label">No active source</span></div><div id="header-actions"><span id="status-pill" data-phase="idle">Connecting…</span><button type="button" class="icon-button" data-action="refresh" aria-label="Refresh configuration">↻</button><button type="button" class="secondary" data-action="open-raw">Open raw</button></div></header>
  <div id="shell">${SECTION_NAV}<main id="configuration-main">${OVERVIEW_SECTION}${RULES_SECTION}${ADOPTION_SECTION}${PATHS_SECTION}${PROJECT_SECTION}</main></div>
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
