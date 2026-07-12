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
      postPreview(selector, byId('bulk-setting').value, projectScope(), false);
    }
    function previewPath() {
      const selector = selectorForSelection();
      const pattern = byId('path-pattern').value.trim();
      if (!selector) { announce('Select rules or a tag in the Rules view first.'); showSection('rules'); return; }
      if (!pattern) { announce('Enter a project-relative path pattern.'); byId('path-pattern').focus(); return; }
      postPreview(selector, byId('path-setting').value, { kind: 'Path', pattern }, false);
    }
    function handleAction(action) {
      if (action === 'refresh') vscode.postMessage({ type: 'refresh' });
      else if (action === 'open-raw') vscode.postMessage({ type: 'openRaw' });
      else if (action === 'disable-all') postPreview({ kind: 'All' }, 'Disabled', projectScope(), false);
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
        byId('rule-search').value = 'has:diagnostics fix:none';
        applyFilter();
        vscode.postMessage({ type: 'occurrences', selector: { kind: 'WithoutSafeFix' }, cursor: undefined, limit: OCCURRENCE_LIMIT });
      } else if (action === 'preview-bulk') previewBulk();
      else if (action === 'clear-selection') {
        selectedCodes.clear();
        activeTag = undefined;
        renderTags();
        applyFilter();
        renderBulkTray();
      } else if (action === 'close-preview') byId('preview-dialog').close();
      else if (action === 'apply-preview') vscode.postMessage({ type: 'apply' });
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
        postPreview({ kind: 'Codes', codes: [target.dataset.ruleSetting] }, target.value, projectScope(), false);
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
