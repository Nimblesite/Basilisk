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
    let editorState = { phase: 'idle', message: '' };
    let snapshot;
    let preview;
    let occurrences;
    let filteredRules = [];
    let activeTag;
    let selectedRuleCode;
    let lastFocusedRule;
    let overlayWasBlocking = false;

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
    // A rule row can request exactly SetRule or RemoveRule ([CONFIGEDITOR-MODEL]).
    function ruleMutation(code, value) {
      return value === NO_ENTRY
        ? { kind: 'RemoveRule', code }
        : { kind: 'SetRule', code, severity: { kind: value } };
    }
    // A tag group can request exactly SetTag or RemoveTag ([CONFIGEDITOR-MODEL]).
    function tagMutation(tag, value) {
      return value === NO_ENTRY
        ? { kind: 'RemoveTag', tag }
        : { kind: 'SetTag', tag, severity: { kind: value } };
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
      const row = document.querySelector('[data-rule-code="' + CSS.escape(lastFocusedRule.code) + '"]');
      if (!row) return;
      const selector = lastFocusedRule.control === 'select' ? 'select' : '.rule-copy button';
      const control = row.querySelector(selector);
      if (control) control.focus();
    }
`;
