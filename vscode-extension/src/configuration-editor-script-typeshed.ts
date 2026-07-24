// Implements [LSPCFGED-TYPESHED] / [LSPCFGED-TYPESHED-DOWNLOAD] standard-library source controls.
/** Typeshed fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_TYPESHED = String.raw`
    // The two mutually exclusive sources and no third ([LSPCFGED-TYPESHED]).
    // The server states which one is ACTIVE (carrying the value that defines
    // it); the copy below is client presentation, not server state.
    const SOURCE_CHOICES = [
      {
        mode: 'ExactCommit',
        label: 'Pinned commit',
        description: 'Freeze one python/typeshed commit so every machine resolves the identical standard library.',
      },
      {
        mode: 'CustomFolder',
        label: 'Custom folder',
        description: 'Use a stdlib tree you manage yourself. Nothing is downloaded.',
      },
    ];
    const COMMIT_PATTERN = /^[0-9a-f]{40}$/i;
    let advancedOpen = false;
    // Which download button is waiting on the server, so the running
    // download's spinner lands on the button that started it and on nothing
    // else ([LSPCFGED-TYPESHED-DOWNLOAD]).
    let pendingDownload;

    function typeshedState() { return snapshot.typeshed; }
    function typeshedSourceMode() { return kind(typeshedState().source, 'ExactCommit'); }
    function typeshedLifecycle() { return kind(typeshedState().status.lifecycle, 'Ready'); }
    function typeshedDownloading() { return typeshedLifecycle() === 'Downloading'; }
    function shortCommit(commit) { return commit ? commit.slice(0, 12) : ''; }
    // The active source is the whole trust story — there are no separate
    // transport or provenance rows ([LSPCFGED-TYPESHED-SERVICE-INFO]).
    function statusRows() {
      const status = typeshedState().status;
      const commit = status.commitIdentity;
      return [
        ['State', typeshedLifecycle()],
        ['Active source', kind(status.activeSource, 'Pending') + (commit ? ' · ' + shortCommit(commit) : '')],
        ['License', kind(status.licenseStatus, 'Unavailable')],
      ];
    }
    // A missing source is a persistent row IN the panel carrying its own fix —
    // never an overlay, never a lock screen ([LSPCFGED-TYPESHED-DOWNLOAD]).
    // The row survives while the fix itself downloads, so the busy button
    // keeps its home until the source settles.
    function noSourceRow() {
      const row = document.createElement('div');
      row.className = 'typeshed-no-source';
      row.setAttribute('role', 'alert');
      row.append(
        textNode('strong', 'NO SOURCE'),
        textNode('span', typeshedState().status.noSourceReason
          || 'The pinned commit is not on this machine; analysis is paused until it is downloaded.'),
        downloadButton('DownloadPinned', 'Download pinned'),
      );
      return row;
    }
    function renderTypeshedStatus() {
      const target = byId('typeshed-status');
      clear(target);
      const summary = document.createElement('dl');
      statusRows().forEach((row) => summary.append(textNode('dt', row[0]), textNode('dd', row[1])));
      target.append(summary);
      typeshedState().status.warnings.forEach((warning) => {
        const row = textNode('p', warning.message, 'typeshed-warning');
        row.dataset.severity = kind(warning.severity, 'Advisory').toLowerCase();
        target.append(row);
      });
      if (typeshedLifecycle() === 'NoSource' || (typeshedDownloading() && pendingDownload === 'DownloadPinned')) {
        target.append(noSourceRow());
      }
    }
    // Both sources stay choosable at all times: reading and writing
    // configuration never waits on the network ([LSPCFGED-TYPESHED-DOWNLOAD]).
    function sourceChoice(choice) {
      const label = document.createElement('label');
      label.className = 'source-choice';
      const input = document.createElement('input');
      input.type = 'radio';
      input.name = 'typeshed-source';
      input.value = choice.mode;
      input.dataset.typeshedSource = choice.mode;
      input.checked = typeshedSourceMode() === choice.mode;
      label.append(input, textNode('span', choice.label), textNode('small', choice.description));
      return label;
    }
    function renderSourceChoices(target) {
      const group = document.createElement('fieldset');
      group.className = 'typeshed-source';
      const legend = document.createElement('legend');
      legend.textContent = 'Source';
      group.append(legend);
      SOURCE_CHOICES.forEach((choice) => group.append(sourceChoice(choice)));
      target.append(group);
    }
    // The active source's own value: a SHA to pin, or a folder to use. No
    // other source's field exists in the DOM ([LSPCFGED-TYPESHED]).
    function renderSourceValue(target) {
      const source = typeshedState().source;
      const mode = kind(source, 'ExactCommit');
      if (mode === 'CustomFolder') { target.append(folderField('TypeshedPath', 'Folder', source.path)); return; }
      target.append(commitField(source.commit));
    }
    function commitField(commit) {
      const field = document.createElement('label');
      field.className = 'typeshed-field';
      field.append(textNode('span', 'Commit'), textNode('small', 'Full 40-character python/typeshed commit SHA.'));
      const input = document.createElement('input');
      input.type = 'text';
      input.value = commit || '';
      input.dataset.typeshedCommit = 'TypeshedCommit';
      input.autocomplete = 'off';
      input.spellcheck = false;
      const error = textNode('small', '', 'field-error');
      error.id = 'typeshed-commit-error';
      error.hidden = true;
      field.append(input, error);
      return field;
    }
    function folderField(key, label, value) {
      const field = document.createElement('label');
      field.className = 'typeshed-field';
      field.append(textNode('span', label));
      const picker = document.createElement('div');
      picker.className = 'path-picker';
      const input = document.createElement('input');
      input.type = 'text';
      input.readOnly = true;
      input.value = value || '';
      input.placeholder = 'Not configured';
      input.dataset.typeshedPath = key;
      const choose = textNode('button', value ? 'Change…' : 'Choose folder…', 'secondary');
      choose.type = 'button';
      choose.dataset.pickTypeshedFolder = key;
      picker.append(input, choose);
      field.append(picker);
      return field;
    }
    // A custom folder downloads nothing, so it has no store folder and the
    // Advanced disclosure does not exist ([LSPCFGED-TYPESHED]).
    function renderStore(target) {
      if (typeshedSourceMode() !== 'ExactCommit') return;
      const details = document.createElement('details');
      details.className = 'typeshed-advanced';
      // Every write re-renders from the fresh snapshot, so the disclosure has
      // to remember itself or it would snap shut under the user's hands.
      details.open = advancedOpen;
      details.addEventListener('toggle', () => { advancedOpen = details.open; });
      const summary = document.createElement('summary');
      summary.textContent = 'Advanced';
      details.append(summary);
      details.append(folderField('TypeshedStorePath', 'Store folder', typeshedState().storeFolder));
      target.append(details);
    }
    function typeshedActionButton(action, label, enabled) {
      const button = textNode('button', label, 'secondary');
      button.type = 'button';
      button.disabled = !enabled;
      button.dataset.typeshedAction = action;
      return button;
    }
    function markBusy(button) {
      button.classList.add('busy');
      button.disabled = true;
      button.setAttribute('aria-busy', 'true');
    }
    // A running download shows progress ON the button that started it; a
    // second download cannot start, and nothing else is blocked
    // ([LSPCFGED-TYPESHED-DOWNLOAD]).
    function downloadButton(action, label) {
      const button = typeshedActionButton(action, label, !typeshedDownloading());
      if (typeshedDownloading() && (pendingDownload || 'DownloadLatest') === action) markBusy(button);
      return button;
    }
    /** The spinner must appear at once — before the server's Downloading state lands. */
    function typeshedActionStarted(action, button) {
      if (action !== 'DownloadLatest' && action !== 'DownloadPinned') return;
      pendingDownload = action;
      markBusy(button);
    }
    // A Typeshed write re-renders this section from the fresh snapshot; the
    // rebuild must not eat the user's focus or caret ([LSPCFGED-TYPESHED]).
    function typeshedFocusSelector(active) {
      if (!active || !active.dataset) return undefined;
      if (active.dataset.typeshedSource) return '[data-typeshed-source="' + active.dataset.typeshedSource + '"]';
      if (active.dataset.typeshedCommit) return '[data-typeshed-commit]';
      if (active.dataset.pickTypeshedFolder) return '[data-pick-typeshed-folder="' + active.dataset.pickTypeshedFolder + '"]';
      if (active.dataset.typeshedAction) return '[data-typeshed-action="' + active.dataset.typeshedAction + '"]';
      return undefined;
    }
    function saveTypeshedFocus() {
      const active = document.activeElement;
      const selector = typeshedFocusSelector(active);
      if (!selector) return undefined;
      const caret = typeof active.selectionStart === 'number';
      return {
        selector,
        start: caret ? active.selectionStart : undefined,
        end: caret ? active.selectionEnd : undefined,
      };
    }
    function restoreTypeshedFocus(saved) {
      if (!saved) return;
      const control = document.querySelector(saved.selector);
      if (!control) return;
      control.focus({ preventScroll: true });
      if (saved.start !== undefined && typeof control.setSelectionRange === 'function') {
        control.setSelectionRange(saved.start, saved.end === undefined ? saved.start : saved.end);
      }
    }
    function renderTypeshedControls() {
      if (!typeshedDownloading()) pendingDownload = undefined;
      const focus = saveTypeshedFocus();
      renderTypeshedStatus();
      const controls = byId('typeshed-controls');
      clear(controls);
      renderSourceChoices(controls);
      renderSourceValue(controls);
      renderStore(controls);
      const actions = byId('typeshed-actions');
      clear(actions);
      actions.append(
        downloadButton('DownloadLatest', 'Download latest'),
        typeshedActionButton('ViewLicense', 'View license', typeshedState().licenseAvailable),
      );
      restoreTypeshedFocus(focus);
    }
    // Choosing a source is one atomic transition ([LSPCFGED-TYPESHED]):
    // pinning clears any folder and a folder clears the pin, so no
    // combination of source values can ever be written. Nothing locks while
    // the mutation round-trips — every control re-renders from the snapshot
    // the write returns.
    function chooseTypeshedSource(mode) {
      if (mode === typeshedSourceMode()) return;
      if (mode === 'ExactCommit') {
        postPreview([typeshedRemove('TypeshedPath')]);
        announce('Using the pinned standard-library commit');
        return;
      }
      vscode.postMessage({ type: 'pickTypeshedFolder', key: 'TypeshedPath' });
    }
    // An invalid SHA is never sent: it is rejected in place, where the user
    // can see why, and the configuration is left untouched.
    function commitEdited(input) {
      const value = input.value.trim();
      const error = byId('typeshed-commit-error');
      if (value !== '' && !COMMIT_PATTERN.test(value)) {
        error.textContent = 'Enter the full 40-character commit SHA (0-9, a-f).';
        error.hidden = false;
        input.setAttribute('aria-invalid', 'true');
        announce('Invalid commit SHA');
        return;
      }
      error.hidden = true;
      input.removeAttribute('aria-invalid');
      postPreview(value === ''
        ? [typeshedRemove('TypeshedCommit')]
        : [typeshedSetText('TypeshedCommit', value), typeshedRemove('TypeshedPath')]);
    }
    /** Every Typeshed control change, routed from the one delegated listener. */
    function typeshedChanged(target) {
      if (target.dataset.typeshedSource) { chooseTypeshedSource(target.value); return true; }
      if (target.dataset.typeshedCommit) { commitEdited(target); return true; }
      return false;
    }
`;
