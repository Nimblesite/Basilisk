// Implements [CONFIGEDITOR-ACCESSIBILITY-SECURITY] webview intent emission.
/** Event and ready-handshake fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_EVENTS = String.raw`
    function selectTag(tag) {
      activeTag = activeTag === tag ? undefined : tag;
      renderTags();
      applyFilter();
      announce(activeTag ? 'Filtered by tag ' + activeTag : 'Tag filter cleared');
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
      if (event.target instanceof HTMLInputElement) return false;
      const keys = ['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End'];
      if (!keys.includes(event.key) || filteredRules.length === 0) return false;
      if (event.target instanceof HTMLSelectElement && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) return false;
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
      const control = event.target instanceof HTMLSelectElement ? 'select' : 'detail';
      lastFocusedRule = { code: filteredRules[target].descriptor.code, control };
      viewport.scrollTop = target * ROW_HEIGHT;
      renderRuleWindow();
      event.preventDefault();
      return true;
    }
    function handleAction(action) {
      if (action === 'refresh') vscode.postMessage({ type: 'refresh' });
      else if (action === 'open-raw') vscode.postMessage({ type: 'openRaw' });
      else if (action === 'load-more-occurrences') loadMoreOccurrences();
      else if (action === 'close-preview') byId('preview-dialog').close();
      else if (action === 'apply-preview' && editorState.phase === 'preview') vscode.postMessage({ type: 'apply' });
      else if (action === 'adopt-workspace') vscode.postMessage({ type: 'adopt', scope: 'workspace' });
      else if (action === 'fix-safe') vscode.postMessage({ type: 'fixSafe' });
      else if (action === 'show-current') {
        showSection('rules');
        byId('rule-search').value = 'has:diagnostics';
        applyFilter();
        byId('rule-search').focus();
      }
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
      const openConfigUri = target.dataset.openConfig;
      if (openConfigUri) { vscode.postMessage({ type: 'openConfigFile', uri: openConfigUri }); return; }
      const folderKey = target.dataset.pickTypeshedFolder;
      if (folderKey) { vscode.postMessage({ type: 'pickTypeshedFolder', key: folderKey }); return; }
      const typeshedAction = target.dataset.typeshedAction;
      if (typeshedAction) {
        // The invoking button goes busy at once; nothing else changes
        // ([LSPCFGED-TYPESHED-DOWNLOAD]).
        typeshedActionStarted(typeshedAction, target);
        vscode.postMessage({ type: 'typeshedAction', action: typeshedAction });
        return;
      }
      const tag = target.dataset.tag;
      if (tag) { selectTag(tag); return; }
      const code = target.dataset.showRule;
      if (code) { showRule(code); return; }
      const docsUri = target.dataset.openDocs;
      if (docsUri) { vscode.postMessage({ type: 'openDocs', uri: docsUri }); return; }
      const findRule = target.dataset.findRule;
      if (findRule) { showRule(findRule); return; }
      if (target.dataset.occurrenceUri) vscode.postMessage(occurrenceMessage(target));
    });
    document.addEventListener('change', (event) => {
      const target = event.target;
      if (target instanceof HTMLInputElement && typeshedChanged(target)) return;
      if (!(target instanceof HTMLSelectElement)) return;
      // One control change = exactly one typed mutation ([CONFIGEDITOR-MODEL]).
      if (target.dataset.ruleEntry) {
        lastFocusedRule = { code: target.dataset.ruleEntry, control: 'select' };
        postPreview([ruleMutation(target.dataset.ruleEntry, target.value)]);
      } else if (target.dataset.tagEntry) {
        postPreview([tagMutation(target.dataset.tagEntry, target.value)]);
      }
    });
    // A dialog dismissed any way at all (button, Escape, backdrop) discards
    // the change: the host returns to the snapshot and every control
    // re-renders from it, so nothing on screen can outlive the decision.
    byId('preview-dialog').addEventListener('close', () => {
      if (editorState.phase === 'preview') vscode.postMessage({ type: 'cancelPreview' });
    });
    byId('rule-search').addEventListener('input', applyFilter);
    byId('rule-viewport').addEventListener('scroll', () => window.requestAnimationFrame(renderRuleWindow), { passive: true });
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
