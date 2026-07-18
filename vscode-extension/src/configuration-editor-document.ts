// Implements [VSIX-CONFIGURATION-EDITOR-HOST] / [CONFIGEDITOR-ACCESSIBILITY-SECURITY].
/** Static, data-free configuration editor document assembled by the hardened host. */

import { buildWebviewDocument } from "./profiler-webview";
import { CONFIGURATION_EDITOR_SCRIPT } from "./configuration-editor-script";
import { CONFIGURATION_EDITOR_STYLES } from "./configuration-editor-styles";

// [CONFIGEDITOR-VSIX-EXPERIENCE]: five navigation views — Overview, Rules,
// Adoption, Path Overrides, Project. Every view renders server-computed state
// (the exact effective state, never a synthetic score). The Rules view is
// tag-first and drives the four rule/tag mutations (tag groups carry the
// tag-entry control; rows carry per-rule entry controls). Overview and Project
// are read-only dashboards; Adoption invokes the standalone adopt / safe-fix
// commands; Path Overrides lists the nested [tool.basilisk] tables the checker
// honors and opens one for editing. There is no preset UI.
const SECTION_NAV = `
  <nav id="section-nav" aria-label="Configuration sections">
    <button type="button" data-section-target="overview"><span aria-hidden="true">⌂</span><span>Overview</span></button>
    <button type="button" data-section-target="rules" aria-current="page"><span aria-hidden="true">☰</span><span>Rules</span></button>
    <button type="button" data-section-target="adoption"><span aria-hidden="true">↗</span><span>Adoption</span></button>
    <button type="button" data-section-target="paths"><span aria-hidden="true">⑂</span><span>Path Overrides</span></button>
    <button type="button" data-section-target="project"><span aria-hidden="true">⚙</span><span>Project</span></button>
  </nav>`;

const OVERVIEW_SECTION = `
  <section id="overview-section" data-section="overview" aria-labelledby="overview-title" hidden>
    <div class="section-heading"><h2 id="overview-title" tabindex="-1">Project policy at a glance</h2><p>Exact effective state from the Basilisk language server — never a synthetic score.</p></div>
    <div class="dashboard-grid">
      <div id="severity-strip" class="card" aria-label="Emitted diagnostics by severity"></div>
      <article class="card" data-accent="orange"><h3>Remaining debt</h3><strong class="stat" id="overview-diagnostics">—</strong><p>diagnostics currently emitted across this workspace</p><button type="button" class="secondary" data-action="show-current">Review occurrences</button></article>
      <article class="card" data-accent="sky"><h3>Adopted rules</h3><strong class="stat" id="overview-adopted">—</strong><p>PEP rules graded below error as deliberate exceptions</p><button type="button" class="secondary" data-section-target="adoption">Open adoption</button></article>
      <article class="card"><h3>Disabled rules</h3><strong class="stat" id="overview-disabled">—</strong><p>analyze rules that resolve to <code>disabled</code></p><button type="button" class="secondary" data-section-target="rules">Open rules</button></article>
    </div>
  </section>`;

const ADOPTION_SECTION = `
  <section id="adoption-section" data-section="adoption" aria-labelledby="adoption-title" hidden>
    <div class="section-heading"><h2 id="adoption-title" tabindex="-1">Strict-first adoption</h2><p>Basilisk runs every PEP rule by default. See what is open, then scope exceptions at the narrowest place they belong — new code stays strict.</p></div>
    <div class="dashboard-grid">
      <article class="card" data-accent="orange"><h3>Rules with open diagnostics</h3><strong class="stat" id="adoption-open-rules">—</strong><p>rules currently reporting in this workspace</p><button type="button" class="secondary" data-action="show-current">Review occurrences</button></article>
      <article class="card" data-accent="sky"><h3>Open diagnostics</h3><strong class="stat" id="adoption-open-diagnostics">—</strong><p>total remaining diagnostics to pay down</p><button type="button" class="secondary" data-section-target="rules">Filter rules</button></article>
      <article class="card wide"><h3>Adopt current debt</h3><p>Adoption records today's error debt as ordinary warning-severity entries in the nearest folder config, so new violations still fail while existing ones are visible. Every change is reviewable in the config file; re-run to tighten.</p><div class="action-row"><button type="button" class="primary" data-action="adopt-workspace">Adopt workspace debt</button><button type="button" class="secondary" data-action="fix-safe">Apply safe fixes</button><button type="button" class="secondary" data-section-target="paths">Scope to a path</button></div></article>
    </div>
  </section>`;

const PATHS_SECTION = `
  <section id="paths-section" data-section="paths" aria-labelledby="paths-title" hidden>
    <div class="section-heading"><h2 id="paths-title" tabindex="-1">Path overrides</h2><p>Nested <code>[tool.basilisk]</code> tables the checker honors for a subtree via nearest-first config discovery. Each is a real folder config; edit one to scope a rule or tag to that directory.</p></div>
    <div class="dashboard-grid">
      <div id="path-override-list" class="wide" aria-label="Discovered path overrides"></div>
    </div>
  </section>`;

const PROJECT_SECTION = `
  <section id="project-section" data-section="project" aria-labelledby="project-title" hidden>
    <div class="section-heading"><h2 id="project-title" tabindex="-1">Project source</h2><p>The active configuration document resolved by the language server. The editor never reads or writes configuration files itself.</p></div>
    <div class="dashboard-grid">
      <article class="card wide" data-accent="sky"><h3>Active configuration</h3><dl id="source-details"></dl><div class="action-row"><button type="button" class="primary" data-action="open-raw">Open raw configuration</button><button type="button" class="secondary" data-action="refresh">Refresh</button></div></article>
      <article class="card wide"><h3>Configuration problems</h3><div id="problem-list"></div></article>
    </div>
  </section>`;

const RULES_SECTION = `
  <section id="rules-section" data-section="rules" aria-labelledby="rules-title">
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
