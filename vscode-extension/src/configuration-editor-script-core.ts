// Implements [CONFIGEDITOR-VSIX-EXPERIENCE].
/** First fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_CORE = String.raw`
  (() => {
    'use strict';
    const vscode = acquireVsCodeApi();
    const ROW_HEIGHT = 112;
    const OVERSCAN = 5;
    const OCCURRENCE_LIMIT = 100;
    const PEP_TAG = 'pep';
    const NO_ENTRY = 'None';
    const SEVERITIES = ['Error', 'Warning', 'Info', 'Disabled'];
    const SECTION_NAMES = ['overview', 'rules', 'adoption', 'paths', 'project'];
    let editorState = { phase: 'idle', message: '' };
    let snapshot;
    let preview;
    let occurrences;
    let filteredRules = [];
    let activeTag;
    let selectedRuleCode;
    let lastFocusedRule;
    let overlayWasBlocking = false;
    // Which navigation view is visible. Rules is the default so the editor
    // opens on the tag-first rule browser exactly as before.
    let activeSection = 'rules';
    // One-shot per webview lifetime: the Configure Severity deep link's
    // focus target is applied on the first snapshot render only, so later
    // state posts never stomp the user's own search/selection.
    let focusRuleConsumed = false;

    function byId(id) { return document.getElementById(id); }
    function clear(node) { node.replaceChildren(); }
    function textNode(tag, text, className) {
      const node = document.createElement(tag);
      node.textContent = text;
      if (className) node.className = className;
      return node;
    }
    function kind(value, fallback) {
      return value && typeof value.kind === 'string' ? value.kind : fallback;
    }
    function compactUri(uri) {
      try {
        const parsed = new URL(uri);
        const path = decodeURIComponent(parsed.pathname);
        const parts = path.split('/').filter(Boolean);
        return parts.slice(-2).join('/') || parsed.host || uri;
      } catch (_error) {
        return uri;
      }
    }
    function formatNumber(value) { return Number(value || 0).toLocaleString(); }
    // entry mirrors the config file exactly: undefined = no per-rule/tag entry.
    function entryValue(entry) { return entry === undefined || entry === null ? NO_ENTRY : kind(entry, NO_ENTRY); }
    function effectiveValue(rule) { return kind(rule.effectiveSeverity, 'Error'); }
    // THE partition ([CHKARCH-COMMANDS]): pep-tagged rules always run and can
    // never be disabled; only analyze rules get a Disabled control.
    function isPepRule(rule) { return rule.descriptor.tags.indexOf(PEP_TAG) !== -1; }
    // A pep-affecting tag entry can never be disabled either: the pep source
    // tag and every PEP category grade only pep rules ([CHKARCH-CONFIG-MODEL]).
    function isPepTag(tag) { return tag.name === PEP_TAG || kind(tag.kind, 'Descriptive') === 'PepCategory'; }
    // Dropdowns list concrete severities only — there is no separate no-entry
    // choice; an analyze rule with no entry is disabled, so the two were one.
    function severityOptions(pep) { return pep ? SEVERITIES.filter((value) => value !== 'Disabled') : SEVERITIES; }
    function ruleSearchText(rule) {
      const descriptor = rule.descriptor;
      return [descriptor.code, descriptor.title, descriptor.summary].concat(descriptor.tags).join(' ').toLowerCase();
    }
    function matchesFacet(rule, token) {
      const lower = token.toLowerCase();
      if (lower.startsWith('tag:')) return rule.descriptor.tags.some((tag) => tag.toLowerCase() === lower.slice(4));
      if (lower.startsWith('severity:')) return effectiveValue(rule).toLowerCase() === lower.slice(9);
      if (lower === 'status:disabled') return effectiveValue(rule) === 'Disabled';
      if (lower === 'status:entry') return rule.entry !== undefined && rule.entry !== null;
      if (lower === 'has:diagnostics') return rule.diagnosticCount > 0;
      return ruleSearchText(rule).includes(lower);
    }
    function applyFilter() {
      if (!snapshot) { filteredRules = []; return; }
      const query = byId('rule-search').value.trim();
      const tokens = query === '' ? [] : query.split(/\s+/);
      filteredRules = snapshot.rules.filter((rule) => {
        const tagMatch = !activeTag || rule.descriptor.tags.includes(activeTag);
        return tagMatch && tokens.every((token) => matchesFacet(rule, token));
      });
      const result = byId('filter-result');
      result.textContent = formatNumber(filteredRules.length) + ' of ' + formatNumber(snapshot.rules.length);
      renderRuleWindow();
    }
    function announce(message) {
      byId('announcer').textContent = '';
      window.setTimeout(() => { byId('announcer').textContent = message; }, 20);
    }
    function postPreview(mutations) {
      vscode.postMessage({ type: 'preview', mutations });
    }
    // A dropdown change always writes an explicit entry ([CONFIGEDITOR-MODEL]):
    // an explicit 'disabled' beats any tag entry, so Disabled always disables.
    function ruleMutation(code, value) {
      return { kind: 'SetRule', code, severity: { kind: value } };
    }
    function tagMutation(tag, value) {
      return { kind: 'SetTag', tag, severity: { kind: value } };
    }
    function selectedRule() {
      return snapshot && snapshot.rules.find((rule) => rule.descriptor.code === selectedRuleCode);
    }
    function saveFocus() {
      const active = document.activeElement;
      const row = active && active.closest ? active.closest('[data-rule-code]') : undefined;
      lastFocusedRule = row ? {
        code: row.getAttribute('data-rule-code'),
        control: active.matches('select') ? 'select' : 'detail',
      } : undefined;
    }
    function restoreFocus() {
      if (!lastFocusedRule) return;
      // Only re-attach focus the row rebuild destroyed (focus fell back to
      // body) — never steal live focus. preventScroll keeps the restored
      // focus from scrolling the stale row back into view, which pinned the
      // viewport and made rules below the fold unreachable.
      const active = document.activeElement;
      if (active && active !== document.body) return;
      const row = document.querySelector('[data-rule-code="' + CSS.escape(lastFocusedRule.code) + '"]');
      if (!row) return;
      const selector = lastFocusedRule.control === 'select' ? 'select' : '.rule-copy button';
      const control = row.querySelector(selector);
      if (control) control.focus({ preventScroll: true });
    }
    // Switch the visible navigation view. Sections carry data-section; the nav
    // buttons carry data-section-target. Rules stays virtualized, so re-measure
    // its viewport once it becomes visible again.
    function showSection(name) {
      if (SECTION_NAMES.indexOf(name) === -1) return;
      activeSection = name;
      document.querySelectorAll('[data-section]').forEach((section) => {
        section.hidden = section.getAttribute('data-section') !== name;
      });
      document.querySelectorAll('#section-nav [data-section-target]').forEach((button) => {
        if (button.getAttribute('data-section-target') === name) button.setAttribute('aria-current', 'page');
        else button.removeAttribute('aria-current');
      });
      if (name === 'rules') window.requestAnimationFrame(renderRuleWindow);
    }
`;
