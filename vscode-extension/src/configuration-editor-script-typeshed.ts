// Implements [LSPCFGED-TYPESHED] standard-library source controls.
/** Typeshed fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_TYPESHED = String.raw`
    // The three mutually exclusive sources. The server states which one is
    // ACTIVE (carrying the value that defines it) and whether a commit can be
    // pinned; the copy below is client presentation, not server state.
    const SOURCE_CHOICES = [
      {
        mode: 'Latest',
        label: 'Latest',
        description: 'Track the newest python/typeshed commit on every acquisition.',
      },
      {
        mode: 'ExactCommit',
        label: 'Pinned commit',
        description: 'Freeze the active commit so every machine resolves the identical standard library.',
      },
      {
        mode: 'CustomFolder',
        label: 'Custom folder',
        description: 'Use a stdlib tree you manage yourself. Nothing is downloaded.',
      },
    ];
    const COMMIT_PATTERN = /^[0-9a-f]{40}$/i;
    let advancedOpen = false;
    const ACQUIRING_HINT = 'A standard library is being acquired; the source is locked until it settles.';

    function typeshedState() { return snapshot.typeshed; }
    function typeshedSourceMode() { return kind(typeshedState().source, 'Latest'); }
    function typeshedAcquiring() {
      return kind(typeshedState().status.lifecycle, 'Ready') === 'Acquiring';
    }
    function shortCommit(commit) { return commit ? commit.slice(0, 12) : ''; }
    // Why a source cannot be chosen right now, or '' when it can. Pinning
    // writes the ACTIVE commit, so it needs one to exist.
    function sourceUnavailable(mode) {
      if (typeshedAcquiring()) return ACQUIRING_HINT;
      if (mode !== 'ExactCommit' || typeshedSourceMode() === 'ExactCommit') return '';
      if (typeshedState().pinnableCommit) return '';
      return typeshedSourceMode() === 'CustomFolder'
        ? 'A custom folder has no upstream commit to pin. Choose Latest first.'
        : 'Available once a downloaded standard library is active.';
    }
    function statusRows() {
      const status = typeshedState().status;
      const lifecycle = kind(status.lifecycle, 'Acquiring');
      const commit = status.commitIdentity;
      return [
        ['State', status.blockedReason ? lifecycle + ' — ' + status.blockedReason : lifecycle],
        ['Active source', kind(status.activeSource, 'Acquiring') + (commit ? ' · ' + shortCommit(commit) : '')],
        ['Delivery', kind(status.transport, 'Pending') + ' · ' + kind(status.provenance, 'Pending')
          + (status.signedRelease ? ' · signed release' : '')],
        ['License', kind(status.licenseStatus, 'Acquiring')],
      ];
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
    }
    function sourceChoice(choice) {
      const label = document.createElement('label');
      label.className = 'source-choice';
      const input = document.createElement('input');
      input.type = 'radio';
      input.name = 'typeshed-source';
      input.value = choice.mode;
      input.dataset.typeshedSource = choice.mode;
      input.checked = typeshedSourceMode() === choice.mode;
      const unavailable = sourceUnavailable(choice.mode);
      input.disabled = unavailable !== '' && !input.checked;
      label.append(input, textNode('span', choice.label));
      label.append(textNode('small', input.disabled ? unavailable : choice.description));
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
      const mode = kind(source, 'Latest');
      if (mode === 'ExactCommit') { target.append(commitField(source.commit)); return; }
      if (mode === 'CustomFolder') { target.append(folderField('TypeshedPath', 'Folder', source.path)); }
    }
    function commitField(commit) {
      const field = document.createElement('label');
      field.className = 'typeshed-field';
      field.append(textNode('span', 'Commit'), textNode('small', 'Full 40-character python/typeshed commit SHA.'));
      const input = document.createElement('input');
      input.type = 'text';
      input.value = commit || '';
      input.disabled = typeshedAcquiring();
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
      choose.disabled = typeshedAcquiring();
      choose.dataset.pickTypeshedFolder = key;
      picker.append(input, choose);
      field.append(picker);
      return field;
    }
    function toggleField(key, label, description, checked) {
      const field = document.createElement('label');
      field.className = 'typeshed-toggle';
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.checked = checked;
      input.disabled = typeshedAcquiring();
      input.dataset.typeshedBoolean = key;
      field.append(input, textNode('span', label), textNode('small', description));
      return field;
    }
    function advancedDownloads(downloads) {
      const details = document.createElement('details');
      details.className = 'typeshed-advanced';
      // Every write re-renders from the fresh snapshot, so the disclosure has
      // to remember itself or it would snap shut under the user's hands.
      details.open = advancedOpen;
      details.addEventListener('toggle', () => { advancedOpen = details.open; });
      const summary = document.createElement('summary');
      summary.textContent = 'Advanced';
      details.append(summary);
      const url = document.createElement('label');
      url.className = 'typeshed-field';
      url.append(textNode('span', 'Archive mirror'), textNode('small', 'HTTPS template containing exactly one {sha}.'));
      const input = document.createElement('input');
      input.type = 'text';
      input.value = downloads.archiveUrl || '';
      input.placeholder = 'https://example.test/{sha}.zip';
      input.disabled = typeshedAcquiring();
      input.dataset.typeshedText = 'TypeshedUrl';
      input.autocomplete = 'off';
      input.spellcheck = false;
      url.append(input);
      details.append(url, folderField('TypeshedCachePath', 'Cache folder', downloads.cacheFolder));
      return details;
    }
    // A user-managed folder downloads nothing, so the server sends no download
    // policy and none of these controls exists.
    function renderDownloads(target) {
      const downloads = typeshedState().downloads;
      if (!downloads) return;
      target.append(
        toggleField('TypeshedCache', 'Reuse downloads', 'Off re-downloads, validates, and discards.', downloads.reuseDownloads),
        toggleField('TypeshedVerify', 'Verify content', 'Attest content to the selected Git tree.', downloads.verifyContent),
        advancedDownloads(downloads),
      );
    }
    function typeshedActionButton(action, label, enabled) {
      const button = textNode('button', label, 'secondary');
      button.type = 'button';
      button.disabled = !enabled;
      button.dataset.typeshedAction = action;
      return button;
    }
    function renderTypeshedControls() {
      renderTypeshedStatus();
      const controls = byId('typeshed-controls');
      clear(controls);
      renderSourceChoices(controls);
      renderSourceValue(controls);
      renderDownloads(controls);
      const actions = byId('typeshed-actions');
      clear(actions);
      actions.append(
        typeshedActionButton('AcquireFresh', 'Acquire fresh', !typeshedAcquiring()),
        typeshedActionButton('ViewLicense', 'View license', typeshedState().licenseAvailable),
      );
    }
    // Choosing a source is one atomic transition, so no combination of source
    // values can ever be written: Latest CLEARS both pins, pinning writes the
    // active commit and clears any folder, and a folder clears the pin.
    function chooseTypeshedSource(mode) {
      // Acquisition is one atomic source transition: nothing may race it.
      if (typeshedAcquiring() || mode === typeshedSourceMode()) return;
      if (mode === 'Latest') {
        postPreview([typeshedRemove('TypeshedCommit'), typeshedRemove('TypeshedPath')]);
        announce('Following the latest python/typeshed commit');
        return;
      }
      if (mode === 'ExactCommit') {
        vscode.postMessage({ type: 'typeshedAction', action: 'PinCurrent' });
        announce('Pinning the active commit');
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
      if (target.dataset.typeshedBoolean) {
        postPreview([typeshedSetBoolean(target.dataset.typeshedBoolean, target.checked)]);
        return true;
      }
      if (target.dataset.typeshedText) {
        const value = target.value.trim();
        postPreview(value === ''
          ? [typeshedRemove(target.dataset.typeshedText)]
          : [typeshedSetText(target.dataset.typeshedText, value)]);
        return true;
      }
      return false;
    }
`;
