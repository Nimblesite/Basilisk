// Implements [CONFIGEDITOR-VSIX-EXPERIENCE].
/** First fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_CORE = String.raw`
  (() => {
    'use strict';
    const vscode = acquireVsCodeApi();
    const ROW_HEIGHT = 112;
    const OVERSCAN = 5;
    const OCCURRENCE_LIMIT = 100;
    const SEVERITY_OPTIONS = ['Inherit', 'Native', 'Error', 'Warning', 'Info', 'Disabled'];
    const SECTION_NAMES = ['overview', 'rules', 'adoption', 'paths', 'project'];
    let editorState = { phase: 'idle', message: '' };
    let snapshot;
    let preview;
    let occurrences;
    let filteredRules = [];
    let activeTag;
    let selectedRuleCode;
    let activeSection = 'rules';
    let lastFocusedRule;
    let overlayWasBlocking = false;
    const selectedCodes = new Set();

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
    function configuredValue(rule) {
      return rule.inherited ? 'Inherit' : kind(rule.configuredSeverity, kind(rule.effectiveSeverity, 'Error'));
    }
    function effectiveValue(rule) { return kind(rule.effectiveSeverity, 'Error'); }
    function ruleSearchText(rule) {
      const descriptor = rule.descriptor;
      return [descriptor.code, descriptor.title, descriptor.summary].concat(descriptor.tags).join(' ').toLowerCase();
    }
    function matchesFacet(rule, token) {
      const lower = token.toLowerCase();
      if (lower.startsWith('tag:')) return rule.descriptor.tags.some((tag) => tag.toLowerCase() === lower.slice(4));
      if (lower.startsWith('severity:')) return effectiveValue(rule).toLowerCase() === lower.slice(9);
      if (lower === 'status:disabled') return effectiveValue(rule) === 'Disabled';
      if (lower === 'status:inherited') return rule.inherited === true;
      if (lower === 'status:changed') return rule.inherited !== true;
      if (lower === 'has:diagnostics') return rule.diagnosticCount > 0;
      if (lower === 'fix:none') return rule.safeFixCount === 0 && rule.unsafeFixCount === 0;
      if (lower === 'fix:without-safe') return rule.diagnosticCount > 0 && rule.safeFixCount === 0;
      if (lower === 'fix:safe') return rule.safeFixCount > 0;
      if (lower === 'fix:unsafe') return rule.unsafeFixCount > 0;
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
    function selectorForSelection() {
      if (selectedCodes.size > 0) return { kind: 'Codes', codes: Array.from(selectedCodes).sort() };
      if (activeTag) return { kind: 'Tags', tags: [activeTag], matchAll: false };
      return undefined;
    }
    function projectScope() { return { kind: 'Project' }; }
    function setting(kindValue) { return { kind: kindValue }; }
    function announce(message) {
      byId('announcer').textContent = '';
      window.setTimeout(() => { byId('announcer').textContent = message; }, 20);
    }
    function postPreview(selector, settingValue, scope) {
      vscode.postMessage({
        type: 'preview',
        mutations: [{ selector, setting: setting(settingValue), scope }],
      });
    }
    function postPreset(presetId) {
      const preset = snapshot && snapshot.presets.find((candidate) => candidate.id === presetId);
      if (!preset) { announce('That preset is no longer available. Refresh configuration.'); return; }
      vscode.postMessage({ type: 'preview', mutations: preset.mutations });
    }
    function showSection(name) {
      if (!SECTION_NAMES.includes(name)) return;
      activeSection = name;
      document.querySelectorAll('[data-section]').forEach((section) => {
        section.hidden = section.getAttribute('data-section') !== name;
      });
      document.querySelectorAll('#section-nav [data-section-target]').forEach((button) => {
        const current = button.getAttribute('data-section-target') === name;
        if (current) button.setAttribute('aria-current', 'page');
        else button.removeAttribute('aria-current');
      });
      if (name === 'rules') window.requestAnimationFrame(renderRuleWindow);
      const heading = document.querySelector('[data-section="' + name + '"] h2');
      if (heading) heading.setAttribute('tabindex', '-1');
    }
    function selectedRule() {
      return snapshot && snapshot.rules.find((rule) => rule.descriptor.code === selectedRuleCode);
    }
    function saveFocus() {
      const active = document.activeElement;
      const row = active && active.closest ? active.closest('[data-rule-code]') : undefined;
      lastFocusedRule = row ? {
        code: row.getAttribute('data-rule-code'),
        control: active.matches('select') ? 'select' : active.matches('input') ? 'checkbox' : 'detail',
      } : undefined;
    }
    function restoreFocus() {
      if (!lastFocusedRule) return;
      const row = document.querySelector('[data-rule-code="' + CSS.escape(lastFocusedRule.code) + '"]');
      if (!row) return;
      const selector = lastFocusedRule.control === 'select' ? 'select' : lastFocusedRule.control === 'checkbox' ? 'input' : '.rule-copy button';
      const control = row.querySelector(selector);
      if (control) control.focus();
    }
`;
