// Implements [CONFIGEDITOR-ACCESSIBILITY-SECURITY] webview intent emission.
/** Event and ready-handshake fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_EVENTS = String.raw`
    function selectTag(tag) {
      activeTag = activeTag === tag ? undefined : tag;
      selectedCodes.clear();
      renderTags();
      applyFilter();
      renderBulkTray();
      announce(activeTag ? 'Filtered by tag ' + activeTag : 'Tag filter cleared');
    }
    function selectRule(code, selected) {
      if (selected) selectedCodes.add(code);
      else selectedCodes.delete(code);
      renderBulkTray();
      const row = document.querySelector('[data-rule-code="' + CSS.escape(code) + '"]');
      if (row) row.dataset.selected = String(selected);
    }
    function showRule(code) {
      selectedRuleCode = code;
      occurrences = undefined;
      renderRuleDetail();
      vscode.postMessage({
        type: 'occurrences',
        selector: { kind: 'Codes', codes: [code] },
        cursor: undefined,
        limit: OCCURRENCE_LIMIT,
      });
    }
    function previewBulk() {
      const selector = selectorForSelection();
      if (!selector) { announce('Select rules or a tag first.'); return; }
      postPreview(selector, byId('bulk-setting').value, projectScope());
    }
    function previewPath() {
      const selector = selectorForSelection();
      const pattern = byId('path-pattern').value.trim();
      if (!selector) { announce('Select rules or a tag in the Rules view first.'); showSection('rules'); return; }
      if (!pattern) { announce('Enter a project-relative path pattern.'); byId('path-pattern').focus(); return; }
      postPreview(selector, byId('path-setting').value, { kind: 'Path', pattern });
    }
    function removePathOverride(pattern) {
      const entry = snapshot && snapshot.pathOverrides.find((candidate) => candidate.pattern === pattern);
      if (!entry || entry.rules.length === 0) { announce('That path override is already empty.'); return; }
      vscode.postMessage({
        type: 'preview',
        mutations: [{
          selector: { kind: 'Codes', codes: entry.rules.map((rule) => rule.ruleCode) },
          setting: { kind: 'Inherit' },
          scope: { kind: 'Path', pattern },
        }],
      });
    }
    function loadMoreOccurrences() {
      if (!selectedRuleCode || !occurrences || !occurrences.nextCursor) return;
      vscode.postMessage({
        type: 'occurrences',
        selector: { kind: 'Codes', codes: [selectedRuleCode] },
        cursor: occurrences.nextCursor,
        limit: OCCURRENCE_LIMIT,
      });
    }
    function moveVirtualRuleFocus(event) {
      const row = event.target instanceof Element ? event.target.closest('[data-rule-code]') : undefined;
      const viewport = byId('rule-viewport');
      if (!row && event.target !== viewport) return false;
      if (event.target instanceof HTMLSelectElement || event.target instanceof HTMLInputElement && event.target.type !== 'checkbox') return false;
      const keys = ['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End'];
      if (!keys.includes(event.key) || filteredRules.length === 0) return false;
      const currentCode = row && row.getAttribute('data-rule-code');
      const current = Math.max(0, filteredRules.findIndex((rule) => rule.descriptor.code === currentCode));
      const page = Math.max(1, Math.floor(viewport.clientHeight / ROW_HEIGHT));
      let target = current;
      if (event.key === 'Home') target = 0;
      else if (event.key === 'End') target = filteredRules.length - 1;
      else if (event.key === 'ArrowUp') target = current - 1;
      else if (event.key === 'ArrowDown') target = current + 1;
      else if (event.key === 'PageUp') target = current - page;
      else if (event.key === 'PageDown') target = current + page;
      target = Math.max(0, Math.min(filteredRules.length - 1, target));
      const active = event.target;
      const control = active instanceof HTMLSelectElement ? 'select'
        : active instanceof HTMLInputElement ? 'checkbox' : 'detail';
      lastFocusedRule = { code: filteredRules[target].descriptor.code, control };
      viewport.scrollTop = target * ROW_HEIGHT;
      renderRuleWindow();
      event.preventDefault();
      return true;
    }
    function handleAction(action) {
      if (action === 'refresh') vscode.postMessage({ type: 'refresh' });
      else if (action === 'fix-safe') vscode.postMessage({ type: 'fixSafe' });
      else if (action === 'open-raw') vscode.postMessage({ type: 'openRaw' });
      else if (action === 'disable-all') postPreview({ kind: 'All' }, 'Disabled', projectScope());
      else if (action === 'show-current') {
        showSection('rules');
        byId('rule-search').value = 'has:diagnostics';
        applyFilter();
        byId('rule-search').focus();
      } else if (action === 'show-suppressions') {
        showSection('rules');
        activeTag = 'suppressions';
        renderTags();
        applyFilter();
      } else if (action === 'show-unfixable') {
        showSection('rules');
        byId('rule-search').value = 'has:diagnostics fix:without-safe';
        applyFilter();
        vscode.postMessage({ type: 'occurrences', selector: { kind: 'WithoutSafeFix' }, cursor: undefined, limit: OCCURRENCE_LIMIT });
      } else if (action === 'disable-unfixable') {
        postPreview({ kind: 'WithoutSafeFix' }, 'Disabled', projectScope());
      } else if (action === 'preview-bulk') previewBulk();
      else if (action === 'load-more-occurrences') loadMoreOccurrences();
      else if (action === 'clear-selection') {
        selectedCodes.clear();
        activeTag = undefined;
        renderTags();
        applyFilter();
        renderBulkTray();
      } else if (action === 'close-preview') byId('preview-dialog').close();
      else if (action === 'apply-preview' && editorState.phase === 'preview') vscode.postMessage({ type: 'apply' });
    }
    function occurrenceMessage(target) {
      return {
        type: 'openOccurrence',
        uri: target.dataset.occurrenceUri,
        line: Number(target.dataset.occurrenceLine),
        character: Number(target.dataset.occurrenceCharacter),
      };
    }
    document.addEventListener('click', (event) => {
      const target = event.target instanceof Element ? event.target.closest('button') : undefined;
      if (!target) return;
      const section = target.dataset.sectionTarget;
      if (section) { showSection(section); return; }
      const action = target.dataset.action;
      if (action) { handleAction(action); return; }
      const removePath = target.dataset.removePath;
      if (removePath) { removePathOverride(removePath); return; }
      const tag = target.dataset.tag;
      if (tag) { selectTag(tag); return; }
      const code = target.dataset.showRule;
      if (code) { showRule(code); return; }
      const docsUri = target.dataset.openDocs;
      if (docsUri) { vscode.postMessage({ type: 'openDocs', uri: docsUri }); return; }
      const findRule = target.dataset.findRule;
      if (findRule) { showRule(findRule); return; }
      const presetId = target.dataset.presetId;
      if (presetId) { postPreset(presetId); return; }
      if (target.dataset.occurrenceUri) vscode.postMessage(occurrenceMessage(target));
    });
    document.addEventListener('change', (event) => {
      const target = event.target;
      if (!(target instanceof HTMLInputElement || target instanceof HTMLSelectElement)) return;
      if (target.dataset.selectRule) selectRule(target.dataset.selectRule, target.checked);
      if (target.dataset.ruleSetting) {
        lastFocusedRule = { code: target.dataset.ruleSetting, control: 'select' };
        postPreview({ kind: 'Codes', codes: [target.dataset.ruleSetting] }, target.value, projectScope());
      }
    });
    byId('rule-search').addEventListener('input', applyFilter);
    byId('rule-viewport').addEventListener('scroll', () => window.requestAnimationFrame(renderRuleWindow), { passive: true });
    byId('path-form').addEventListener('submit', (event) => { event.preventDefault(); previewPath(); });
    window.addEventListener('resize', () => window.requestAnimationFrame(renderRuleWindow));
    window.addEventListener('message', (event) => {
      if (!event.data || event.data.type !== 'state') return;
      renderState(event.data.state);
    });
    document.addEventListener('keydown', (event) => {
      if (moveVirtualRuleFocus(event)) return;
      const typing = event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement || event.target instanceof HTMLTextAreaElement;
      if (event.key === '/' && !typing) {
        event.preventDefault();
        showSection('rules');
        byId('rule-search').focus();
      }
      if (event.key === 'Escape' && byId('preview-dialog').open) byId('preview-dialog').close();
    });
    showSection(activeSection);
    vscode.postMessage({ type: 'ready' });
  })();
`;
