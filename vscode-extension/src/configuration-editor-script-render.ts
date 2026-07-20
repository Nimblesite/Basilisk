// Implements [CONFIGEDITOR-VSIX-EXPERIENCE] rendering from LSP-owned state.
/** Render fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_RENDER = String.raw`
    function entryOption(value, selected) {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = value;
      option.selected = value === selected;
      return option;
    }
    function makeTagEntrySelect(tag) {
      const label = document.createElement('label');
      label.append(textNode('span', 'Entry for tag ' + tag.name, 'sr-only'));
      const select = document.createElement('select');
      select.className = 'severity-select';
      select.dataset.tagEntry = tag.name;
      // No entry shows what no entry resolves to ([CHKARCH-CONFIG-MODEL]):
      // pep-affecting tags run at error, everything else does not run.
      const current = entryValue(tag.entry) === NO_ENTRY
        ? (isPepTag(tag) ? 'Error' : 'Disabled')
        : entryValue(tag.entry);
      select.dataset.severity = current;
      // Tag-entry control: error/warning/info — plus disabled only where the
      // server would accept it (never on a tag that grades pep rules).
      severityOptions(isPepTag(tag)).forEach((value) => select.append(entryOption(value, current)));
      label.append(select);
      return label;
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
          const row = document.createElement('div');
          row.className = 'tag-row';
          const button = document.createElement('button');
          button.type = 'button';
          button.className = 'tag-button';
          button.dataset.tag = tag.name;
          button.setAttribute('aria-pressed', String(activeTag === tag.name));
          button.append(textNode('span', tag.name), textNode('small', formatNumber(tag.ruleCount) + ' · ' + formatNumber(tag.diagnosticCount)));
          row.append(button, makeTagEntrySelect(tag));
          section.append(row);
        });
        list.append(section);
      });
    }
    function makeRuleEntrySelect(rule) {
      const label = document.createElement('label');
      label.append(textNode('span', 'Entry for ' + rule.descriptor.code, 'sr-only'));
      const select = document.createElement('select');
      select.className = 'severity-select';
      select.dataset.ruleEntry = rule.descriptor.code;
      select.dataset.severity = effectiveValue(rule);
      // No entry shows the resolved severity — for an untouched analyze rule
      // that IS Disabled ([CHKARCH-CONFIG-MODEL] resolution step 3).
      const current = entryValue(rule.entry) === NO_ENTRY ? effectiveValue(rule) : entryValue(rule.entry);
      // pep rows: error/warning/info — no Disabled control exists for them
      // ([CHKARCH-CONFIG-MODEL]); analyze rows also offer Disabled.
      severityOptions(isPepRule(rule)).forEach((value) => select.append(entryOption(value, current)));
      label.append(select);
      return label;
    }
    function makeRuleRow(rule, index) {
      const descriptor = rule.descriptor;
      const row = document.createElement('article');
      row.className = 'rule-row';
      row.setAttribute('role', 'listitem');
      row.dataset.ruleCode = descriptor.code;
      row.setAttribute('aria-posinset', String(index + 1));
      row.setAttribute('aria-setsize', String(filteredRules.length));
      row.style.top = String(index * ROW_HEIGHT) + 'px';
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
      chips.append(textNode('span', 'effective ' + effectiveValue(rule) + ' · ' + formatNumber(rule.diagnosticCount) + ' issues', 'metrics'));
      copy.append(detail, summary, chips);
      row.append(copy, makeRuleEntrySelect(rule));
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
        ['Entry', entryValue(rule.entry) === 'None' ? 'No entry' : entryValue(rule.entry)],
        ['Effective', effectiveValue(rule)],
        ['Scope', isPepRule(rule) ? 'check · always runs' : 'analyze'],
        ['Diagnostics', formatNumber(rule.diagnosticCount)],
        ['Tags', descriptor.tags.join(', ')],
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
        occurrences.items.filter((item) => item.code === descriptor.code).forEach((item) => {
          const button = textNode('button', compactUri(item.uri) + ':' + String(item.range.start.line + 1), 'occurrence');
          button.type = 'button';
          button.dataset.occurrenceUri = item.uri;
          button.dataset.occurrenceLine = String(item.range.start.line);
          button.dataset.occurrenceCharacter = String(item.range.start.character);
          button.append(textNode('small', kind(item.severity, 'Error')));
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
    function renderSource() {
      byId('root-label').textContent = compactUri(snapshot.rootUri);
      byId('source-label').textContent = compactUri(snapshot.configUri) + ' · revision ' + snapshot.revision;
    }
    // Configure Severity deep link ([CONFIGEDITOR-VSIX-EXPERIENCE]): focus
    // the requested rule once — prefill the search filter with its code,
    // scroll its row into the virtual window, and open its detail panel.
    function consumeFocusRule() {
      const code = editorState.focusRule;
      if (focusRuleConsumed || !code || !snapshot) return;
      if (!snapshot.rules.some((rule) => rule.descriptor.code === code)) return;
      focusRuleConsumed = true;
      byId('rule-search').value = code;
      applyFilter();
      const index = filteredRules.findIndex((rule) => rule.descriptor.code === code);
      if (index >= 0) byId('rule-viewport').scrollTop = index * ROW_HEIGHT;
      renderRuleWindow();
      showRule(code);
      announce('Focused rule ' + code);
    }
    // Overview/Adoption/Project/Path Overrides render exact server-computed
    // snapshot state (snapshot.debt / .source / .problems / .pathOverrides) —
    // never client arithmetic dressed up as a score ([CONFIGEDITOR-VSIX-EXPERIENCE]).
    function renderSeverityStrip() {
      const strip = byId('severity-strip');
      clear(strip);
      const debt = snapshot.debt;
      [['Error', debt.errorDiagnostics], ['Warning', debt.warningDiagnostics], ['Info', debt.infoDiagnostics], ['Total', debt.remainingDiagnostics]]
        .forEach(([label, value]) => {
          const cell = document.createElement('div');
          cell.append(textNode('strong', formatNumber(value)), textNode('span', label));
          strip.append(cell);
        });
    }
    function renderOverview() {
      renderSeverityStrip();
      byId('overview-diagnostics').textContent = formatNumber(snapshot.debt.remainingDiagnostics);
      byId('overview-adopted').textContent = formatNumber(snapshot.debt.adoptedRules);
      byId('overview-disabled').textContent = formatNumber(snapshot.debt.disabledRules);
    }
    function renderAdoption() {
      const openRules = snapshot.rules.filter((rule) => rule.diagnosticCount > 0).length;
      byId('adoption-open-rules').textContent = formatNumber(openRules);
      byId('adoption-open-diagnostics').textContent = formatNumber(snapshot.debt.remainingDiagnostics);
    }
    function pathSettingRow(label, severity) {
      const item = document.createElement('li');
      item.append(textNode('code', label), textNode('span', kind(severity, 'Error')));
      return item;
    }
    function renderPaths() {
      const list = byId('path-override-list');
      clear(list);
      const overrides = snapshot.pathOverrides || [];
      if (overrides.length === 0) {
        list.append(textNode('p', 'No path overrides. Project policy applies everywhere. Add a nested pyproject.toml [tool.basilisk] table to scope rules to a subtree.', 'empty-state'));
        return;
      }
      overrides.forEach((entry) => {
        const card = document.createElement('article');
        card.className = 'path-override-card';
        const header = document.createElement('div');
        header.className = 'path-override-head';
        header.append(textNode('h3', entry.path || '.'));
        const open = textNode('button', 'Open configuration file', 'secondary');
        open.type = 'button';
        open.dataset.openConfig = entry.configUri;
        header.append(open);
        card.append(header);
        const rows = document.createElement('ul');
        entry.rules.forEach((rule) => rows.append(pathSettingRow(rule.code, rule.severity)));
        entry.tags.forEach((tag) => rows.append(pathSettingRow('tag:' + tag.tag, tag.severity)));
        card.append(rows);
        list.append(card);
      });
    }
    function renderProject() {
      const dl = byId('source-details');
      clear(dl);
      const source = snapshot.source;
      const facts = [
        ['Root', compactUri(snapshot.rootUri)],
        ['Source', compactUri(source.uri)],
        ['Revision', snapshot.revision],
        ['On disk', source.exists ? 'Yes' : 'Created on first change'],
        ['Writable', source.readOnly ? 'Read-only' : 'Writable'],
      ];
      facts.forEach(([name, value]) => dl.append(textNode('dt', name), textNode('dd', value)));
      renderTypeshedControls();
      const problemList = byId('problem-list');
      clear(problemList);
      const problems = snapshot.problems || [];
      if (problems.length === 0) {
        problemList.className = 'empty-state';
        problemList.textContent = 'No configuration problems.';
        return;
      }
      problemList.className = '';
      problems.forEach((problem) => {
        const item = document.createElement('p');
        item.className = 'problem-row';
        item.append(textNode('strong', problem.code), document.createTextNode(' ' + problem.message));
        problemList.append(item);
      });
    }
    function renderSnapshot() {
      if (!snapshot) return;
      saveFocus();
      renderSource();
      renderOverview();
      renderAdoption();
      renderPaths();
      renderProject();
      renderTags();
      applyFilter();
      consumeFocusRule();
      renderRuleDetail();
      window.requestAnimationFrame(restoreFocus);
    }
    function impactCell(before, after, label) {
      const cell = document.createElement('div');
      cell.append(
        textNode('strong', formatNumber(before) + ' → ' + formatNumber(after)),
        textNode('span', label),
      );
      return cell;
    }
    function renderPreview() {
      if (!preview) return;
      const impact = preview.impact;
      const grid = byId('impact-grid');
      clear(grid);
      // Complete before/after partition by the three emitting severities.
      grid.append(
        impactCell(impact.errorsBefore, impact.errorsAfter, 'errors'),
        impactCell(impact.warningsBefore, impact.warningsAfter, 'warnings'),
        impactCell(impact.infosBefore, impact.infosAfter, 'infos'),
      );
      const changes = byId('preview-changes');
      clear(changes);
      preview.changes.forEach((change) => {
        const row = document.createElement('div');
        row.className = 'preview-change';
        row.append(
          textNode('code', change.code),
          textNode('strong', kind(change.before, 'Error') + ' → ' + kind(change.after, 'Error')),
        );
        changes.append(row);
      });
      preview.typeshedChanges.forEach((change) => {
        const row = document.createElement('div');
        row.className = 'preview-change';
        const before = change.before && Object.prototype.hasOwnProperty.call(change.before, 'value')
          ? String(change.before.value)
          : 'default';
        const after = change.after && Object.prototype.hasOwnProperty.call(change.after, 'value')
          ? String(change.after.value)
          : 'default';
        row.append(textNode('code', kind(change.key, 'Typeshed')), textNode('strong', before + ' → ' + after));
        changes.append(row);
      });
      if (preview.changes.length === 0 && preview.typeshedChanges.length === 0) {
        changes.append(textNode('p', 'No effective configuration changes.', 'empty-state'));
      }
      const dialog = byId('preview-dialog');
      if (!dialog.open) dialog.showModal();
      announce('Preview ready: ' + formatNumber(preview.changes.length + preview.typeshedChanges.length) + ' setting(s) change');
    }
    function renderOverlay() {
      const overlay = byId('state-overlay');
      const blocking = !snapshot || ['loading', 'applying', 'error', 'conflict', 'unsupported'].includes(editorState.phase);
      overlay.hidden = !blocking;
      byId('shell').inert = blocking;
      document.querySelector('body > header').inert = blocking;
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
        loading: 'Reading project configuration', applying: 'Applying configuration', error: 'Configuration unavailable',
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
