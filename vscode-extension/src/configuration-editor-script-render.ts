// Implements [CONFIGEDITOR-VSIX-EXPERIENCE] rendering from LSP-owned state.
/** Render fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_RENDER = String.raw`
    function renderSeverityStrip() {
      const strip = byId('severity-strip');
      clear(strip);
      const counts = { Error: 0, Warning: 0, Info: 0, Disabled: 0, Inherited: 0 };
      snapshot.rules.forEach((rule) => {
        counts[effectiveValue(rule)] += 1;
        if (rule.inherited) counts.Inherited += 1;
      });
      Object.entries(counts).forEach(([label, count]) => {
        const cell = document.createElement('div');
        cell.append(textNode('strong', formatNumber(count)), textNode('span', label));
        strip.append(cell);
      });
    }
    function renderTags() {
      const list = byId('tag-list');
      clear(list);
      const groups = [
        { kind: 'Provenance', label: 'Sources' },
        { kind: 'PepCategory', label: 'PEP categories' },
        { kind: 'Descriptive', label: 'Policy tags' },
      ];
      groups.forEach((group) => {
        const tags = snapshot.tags
          .filter((tag) => kind(tag.kind, 'Descriptive') === group.kind)
          .slice()
          .sort((left, right) => left.name.localeCompare(right.name));
        if (tags.length === 0) return;
        const section = document.createElement('section');
        section.className = 'tag-group';
        section.append(textNode('h3', group.label));
        tags.forEach((tag) => {
          const button = document.createElement('button');
          button.type = 'button';
          button.className = 'tag-button';
          button.dataset.tag = tag.name;
          button.setAttribute('aria-pressed', String(activeTag === tag.name));
          button.append(textNode('span', tag.name), textNode('small', formatNumber(tag.ruleCount) + ' · ' + formatNumber(tag.diagnosticCount)));
          section.append(button);
        });
        list.append(section);
      });
    }
    function makeSeveritySelect(rule) {
      const label = document.createElement('label');
      label.append(textNode('span', 'Severity for ' + rule.descriptor.code, 'sr-only'));
      const select = document.createElement('select');
      select.className = 'severity-select';
      select.dataset.ruleSetting = rule.descriptor.code;
      select.dataset.severity = effectiveValue(rule);
      SEVERITY_OPTIONS.forEach((optionValue) => {
        const option = document.createElement('option');
        option.value = optionValue;
        option.textContent = optionValue === 'Inherit' ? 'Inherited · reset'
          : optionValue === 'Native' ? 'Native severity' : optionValue;
        select.append(option);
      });
      select.value = configuredValue(rule);
      select.disabled = snapshot.source.readOnly === true || snapshot.problems.length > 0;
      label.append(select);
      return label;
    }
    function makeRuleRow(rule, index) {
      const descriptor = rule.descriptor;
      const row = document.createElement('article');
      row.className = 'rule-row';
      row.setAttribute('role', 'listitem');
      row.dataset.ruleCode = descriptor.code;
      row.dataset.selected = String(selectedCodes.has(descriptor.code));
      row.setAttribute('aria-posinset', String(index + 1));
      row.setAttribute('aria-setsize', String(filteredRules.length));
      row.style.top = String(index * ROW_HEIGHT) + 'px';
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.checked = selectedCodes.has(descriptor.code);
      checkbox.dataset.selectRule = descriptor.code;
      checkbox.setAttribute('aria-label', 'Select ' + descriptor.code);
      const copy = document.createElement('div');
      copy.className = 'rule-copy';
      const detail = document.createElement('button');
      detail.type = 'button';
      detail.dataset.showRule = descriptor.code;
      detail.append(textNode('strong', descriptor.code), textNode('span', descriptor.title, 'title'));
      const summary = textNode('p', descriptor.summary);
      const chips = document.createElement('div');
      chips.className = 'chip-list';
      descriptor.tags.forEach((tag) => chips.append(textNode('span', tag, 'chip')));
      chips.append(textNode('span', formatNumber(rule.diagnosticCount) + ' issues · ' + formatNumber(rule.safeFixCount) + ' safe fixes', 'metrics'));
      copy.append(detail, summary, chips);
      row.append(checkbox, copy, makeSeveritySelect(rule));
      return row;
    }
    function renderRuleWindow() {
      const viewport = byId('rule-viewport');
      const spacer = byId('rule-spacer');
      const windowNode = byId('rule-window');
      if (!viewport || !spacer || !windowNode) return;
      spacer.style.height = String(filteredRules.length * ROW_HEIGHT) + 'px';
      const visible = Math.ceil(viewport.clientHeight / ROW_HEIGHT);
      const start = Math.max(0, Math.floor(viewport.scrollTop / ROW_HEIGHT) - OVERSCAN);
      const end = Math.min(filteredRules.length, start + visible + OVERSCAN * 2);
      clear(windowNode);
      for (let index = start; index < end; index += 1) windowNode.append(makeRuleRow(filteredRules[index], index));
      restoreFocus();
    }
    function renderBulkTray() {
      const tray = byId('bulk-tray');
      const count = selectedCodes.size;
      const hasTag = !!activeTag;
      tray.hidden = count === 0 && !hasTag;
      byId('selection-count').textContent = count > 0
        ? formatNumber(count) + ' rule' + (count === 1 ? '' : 's') + ' selected'
        : 'Tag · ' + activeTag;
    }
    function renderRuleDetail() {
      const rule = selectedRule();
      const empty = byId('detail-empty');
      const content = byId('detail-content');
      if (!rule) { empty.hidden = false; content.hidden = true; return; }
      empty.hidden = true;
      content.hidden = false;
      clear(content);
      const descriptor = rule.descriptor;
      const heading = textNode('h3', descriptor.code + ' · ' + descriptor.title);
      const summary = textNode('p', descriptor.summary);
      const dl = document.createElement('dl');
      const facts = [
        ['Effective', effectiveValue(rule)],
        ['Configured', rule.inherited ? 'Inherited' : configuredValue(rule)],
        ['Default', kind(descriptor.defaultSeverity, 'Error') + (descriptor.defaultEnabled ? ' · on' : ' · opt-in')],
        ['Diagnostics', formatNumber(rule.diagnosticCount) + ' in ' + formatNumber(rule.affectedFileCount) + ' files'],
        ['Fixes', formatNumber(rule.safeFixCount) + ' safe · ' + formatNumber(rule.unsafeFixCount) + ' unsafe'],
        ['Adoption debt', formatNumber(rule.adoptionExceptionCount)],
      ];
      facts.forEach(([name, value]) => dl.append(textNode('dt', name), textNode('dd', value)));
      const actions = document.createElement('div');
      actions.className = 'action-row';
      const docs = textNode('button', 'Open rule guide', 'secondary');
      docs.type = 'button';
      docs.dataset.openDocs = descriptor.docsUrl;
      const find = textNode('button', 'Find occurrences', 'primary');
      find.type = 'button';
      find.dataset.findRule = descriptor.code;
      actions.append(find, docs);
      const occurrenceList = document.createElement('div');
      occurrenceList.id = 'occurrence-list';
      if (occurrences && occurrences.items.length > 0) {
        occurrences.items.filter((item) => item.ruleCode === descriptor.code).forEach((item) => {
          const button = textNode('button', compactUri(item.uri) + ':' + String(item.range.start.line + 1), 'occurrence');
          button.type = 'button';
          button.dataset.occurrenceUri = item.uri;
          button.dataset.occurrenceLine = String(item.range.start.line);
          button.dataset.occurrenceCharacter = String(item.range.start.character);
          button.append(textNode('small', kind(item.effectiveSeverity, 'Error') + (item.fixSafety ? ' · ' + kind(item.fixSafety, '') + ' fix' : ' · no fix')));
          occurrenceList.append(button);
        });
      }
      if (editorState.occurrencesLoading) {
        occurrenceList.append(textNode('p', 'Loading occurrences…', 'empty-state'));
      } else if (occurrences && occurrences.nextCursor) {
        const more = textNode('button', 'Load more occurrences', 'secondary');
        more.type = 'button';
        more.dataset.action = 'load-more-occurrences';
        occurrenceList.append(more);
      } else if (!occurrences || occurrences.items.length === 0) {
        occurrenceList.append(textNode('p', 'No loaded occurrences.', 'empty-state'));
      }
      content.append(heading, summary, dl, actions, occurrenceList);
    }
    function renderProblems() {
      const banner = byId('problem-banner');
      const problemList = byId('problem-list');
      clear(problemList);
      if (snapshot.problems.length === 0) {
        banner.hidden = true;
        problemList.append(textNode('p', 'No configuration problems.', 'empty-state'));
        return;
      }
      banner.hidden = false;
      banner.textContent = formatNumber(snapshot.problems.length) + ' configuration problem(s) make this source read-only until fixed.';
      snapshot.problems.forEach((problem) => {
        const item = document.createElement('p');
        item.append(textNode('strong', problem.code + ' · '), document.createTextNode(problem.message));
        problemList.append(item);
      });
    }
    function renderSource() {
      const source = snapshot.source;
      byId('source-label').textContent = (source.exists ? compactUri(source.uri) : 'New pyproject.toml') + (source.readOnly ? ' · read-only' : '');
      const dl = byId('source-details');
      clear(dl);
      const rows = [
        ['Root', compactUri(snapshot.rootUri)],
        ['Source', source.exists ? source.uri : 'Will create pyproject.toml'],
        ['Format', kind(source.format, 'PyprojectToml')],
        ['Revision', snapshot.revision],
        ['Writable', source.readOnly ? 'No' : 'Yes'],
        ['Shadowed', source.shadowedSources.length ? source.shadowedSources.join(', ') : 'None'],
      ];
      rows.forEach(([name, value]) => dl.append(textNode('dt', name), textNode('dd', value)));
    }
    function renderDebt() {
      byId('debt-diagnostics').textContent = formatNumber(snapshot.debt.remainingDiagnostics);
      byId('debt-adoptions').textContent = formatNumber(snapshot.debt.adoptionExceptions);
      byId('debt-suppressions').textContent = formatNumber(snapshot.debt.suppressionDiagnostics);
    }
    function renderSafeFixAction() {
      const button = byId('fix-safe-button');
      const count = snapshot.rules.reduce((total, rule) => total + rule.safeFixCount, 0);
      button.textContent = count === 1 ? 'Apply 1 safe fix' : 'Apply ' + formatNumber(count) + ' safe fixes';
      const busy = ['previewing', 'applying'].includes(editorState.phase);
      button.disabled = count === 0 || snapshot.source.readOnly === true
        || snapshot.problems.length > 0 || busy;
      button.title = count === 0 ? 'No safe fixes are currently available.'
        : snapshot.source.readOnly ? 'The active configuration source is read-only.'
          : busy ? 'Wait for the current operation to finish.' : '';
    }
    function renderPresets() {
      const targets = [byId('preset-list'), byId('adoption-presets')];
      targets.forEach((target) => {
        clear(target);
        snapshot.presets.forEach((preset) => {
          const button = document.createElement('button');
          button.type = 'button';
          button.className = 'preset-card';
          button.dataset.presetId = preset.id;
          button.append(
            textNode('strong', preset.name),
            textNode('span', preset.summary),
            textNode('small', 'Preview explicit config changes'),
          );
          target.append(button);
        });
        if (snapshot.presets.length === 0) target.append(textNode('p', 'No policy presets advertised by this server.', 'empty-state'));
      });
    }
    function renderPathOverrides() {
      const list = byId('path-override-list');
      clear(list);
      if (snapshot.pathOverrides.length === 0) {
        list.append(textNode('p', 'No path overrides. Project policy applies everywhere.', 'empty-state'));
        return;
      }
      snapshot.pathOverrides.forEach((entry) => {
        const card = document.createElement('article');
        card.className = 'path-override-card';
        const heading = textNode('h3', entry.pattern);
        const badge = textNode('span', entry.adoption ? 'Adoption debt' : 'Path policy', 'chip');
        const rules = document.createElement('ul');
        entry.rules.forEach((rule) => {
          const item = document.createElement('li');
          item.append(textNode('code', rule.ruleCode), textNode('span', kind(rule.severity, 'Error')));
          rules.append(item);
        });
        const remove = textNode('button', 'Preview removing override…', 'secondary');
        remove.type = 'button';
        remove.dataset.removePath = entry.pattern;
        remove.disabled = snapshot.source.readOnly === true || snapshot.problems.length > 0;
        card.append(heading, badge, rules, remove);
        list.append(card);
      });
    }
    function renderSnapshot() {
      if (!snapshot) return;
      saveFocus();
      byId('root-label').textContent = compactUri(snapshot.rootUri);
      renderSeverityStrip();
      renderDebt();
      renderSafeFixAction();
      renderPresets();
      renderSource();
      renderProblems();
      renderTags();
      renderPathOverrides();
      applyFilter();
      renderBulkTray();
      renderRuleDetail();
      window.requestAnimationFrame(restoreFocus);
    }
    function impactCell(value, label) {
      const cell = document.createElement('div');
      cell.append(textNode('strong', formatNumber(value)), textNode('span', label));
      return cell;
    }
    function renderPreview() {
      if (!preview) return;
      const impact = preview.impact;
      const diagnosticDelta = impact.diagnosticsBefore - impact.diagnosticsAfter;
      const grid = byId('impact-grid');
      clear(grid);
      grid.append(
        impactCell(impact.changedRules, 'rules changed'),
        impactCell(impact.errorsAfter, 'errors after'),
        impactCell(impact.warningsAfter, 'warnings after'),
        impactCell(Math.abs(diagnosticDelta), diagnosticDelta >= 0 ? 'diagnostics removed' : 'diagnostics added'),
        impactCell(impact.enabledRules, 'rules enabled'),
        impactCell(impact.disabledRules, 'rules disabled'),
      );
      byId('expanded-rules').textContent = 'Selector expanded to ' + formatNumber(preview.expandedRuleCodes.length)
        + ' rules; ' + formatNumber(preview.changes.length) + ' persisted entries will change.';
      const changes = byId('preview-changes');
      clear(changes);
      preview.changes.forEach((change) => {
        const row = document.createElement('div');
        row.className = 'preview-change';
        const scope = kind(change.scope, 'Project') === 'Path'
          ? 'Path · ' + change.scope.pattern
          : 'Project';
        const previous = kind(change.previousSetting, 'Inherit') === 'Inherit'
          ? 'Inherited'
          : kind(change.previousSetting, 'Error');
        const result = kind(change.resultingSetting, 'Inherit') === 'Inherit'
          ? 'Inherited (reset)'
          : kind(change.resultingSetting, 'Error');
        row.append(
          textNode('code', change.ruleCode),
          textNode('span', scope),
          textNode('strong', previous + ' → ' + result),
        );
        changes.append(row);
      });
      if (preview.changes.length === 0) {
        changes.append(textNode('p', 'No persisted entries would change.', 'empty-state'));
      }
      const problems = byId('preview-problems');
      clear(problems);
      preview.problems.forEach((problem) => problems.append(textNode('p', problem.code + ' · ' + problem.message)));
      const applyButton = document.querySelector('[data-action="apply-preview"]');
      const blocked = preview.problems.length > 0 || snapshot.source.readOnly === true;
      applyButton.disabled = blocked;
      applyButton.title = snapshot.source.readOnly
        ? 'The active configuration source is read-only.'
        : preview.problems.length > 0 ? 'Resolve preview problems before applying.' : '';
      const dialog = byId('preview-dialog');
      if (!dialog.open) dialog.showModal();
      announce('Preview ready for ' + formatNumber(preview.expandedRuleCodes.length) + ' rules');
    }
    function renderOverlay() {
      const overlay = byId('state-overlay');
      const blocking = !snapshot || ['loading', 'applying', 'error', 'conflict', 'unsupported'].includes(editorState.phase);
      overlay.hidden = !blocking;
      byId('shell').inert = blocking;
      document.querySelector('body > header').inert = blocking;
      byId('bulk-tray').inert = blocking;
      document.querySelector('main').setAttribute('aria-busy', String(blocking));
      if (!blocking) {
        if (overlayWasBlocking) {
          const recovery = activeSection === 'rules'
            ? byId('rule-search')
            : document.querySelector('[data-section="' + activeSection + '"] h2');
          if (recovery) window.requestAnimationFrame(() => recovery.focus());
        }
        overlayWasBlocking = false;
        return;
      }
      const previewDialog = byId('preview-dialog');
      if (previewDialog.open) previewDialog.close();
      const titles = {
        loading: 'Reading project policy', applying: 'Applying configuration', error: 'Configuration unavailable',
        conflict: 'The project changed', unsupported: 'Update Basilisk to continue', idle: 'Connecting to Basilisk',
      };
      byId('state-title').textContent = titles[editorState.phase] || 'Working…';
      byId('state-message').textContent = editorState.message || 'Waiting for the language server.';
      byId('state-symbol').textContent = editorState.phase === 'conflict' ? '↻' : editorState.phase === 'error' ? '!' : 'B';
      byId('state-action').hidden = !['error', 'conflict'].includes(editorState.phase);
      byId('state-open-raw').hidden = !editorState.repairUri;
      if (!overlayWasBlocking) window.requestAnimationFrame(() => overlay.focus());
      overlayWasBlocking = true;
    }
    function renderState(nextState) {
      editorState = nextState || { phase: 'error', message: 'Invalid editor state' };
      snapshot = editorState.snapshot;
      preview = editorState.preview;
      occurrences = editorState.occurrences;
      const status = byId('status-pill');
      status.dataset.phase = editorState.phase;
      status.textContent = editorState.message || editorState.phase;
      if (snapshot) renderSnapshot();
      renderOverlay();
      if (preview && editorState.phase === 'preview') renderPreview();
      const dialog = byId('preview-dialog');
      if (editorState.phase !== 'preview' && dialog.open) dialog.close();
      announce(editorState.message || editorState.phase);
    }
`;
