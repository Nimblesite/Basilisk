// The page-side driver prelude every configuration-editor DOM scenario shares.
/**
 * Helpers the injected driver script runs INSIDE the webview: waiting on
 * observable consequences rather than fixed sleeps, and one `probe()` that
 * reads every observable fact about the Project view's setting panels
 * ([LSPCFGED-TYPESHED], [LSPCFGED-CACHE]) at an instant.
 *
 * Split from `webview-dom-harness.ts` (which owns the extension-host half) to
 * keep both files under the repository size ceiling.
 */

export const DRIVER_PRELUDE = String.raw`
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const settleDelay = 120;
  const report = (result) => window.__realApi.postMessage(Object.assign({ type: 'domTestResult' }, result));
  const el = (selector) => document.querySelector(selector);
  const all = (selector) => Array.from(document.querySelectorAll(selector));
  const text = (node) => (node && node.textContent ? node.textContent.trim() : null);
  const waitFor = async (selector, tries) => {
    for (let attempt = 0; attempt < (tries || 100); attempt += 1) {
      if (el(selector)) return true;
      await sleep(25);
    }
    return false;
  };
  // Wait for an OBSERVABLE consequence instead of guessing how long the
  // extension host will take. Every webview interaction costs two IPC round
  // trips through the host's single event loop plus a full re-render, and that
  // loop is shared with every other suite in the run — a language client
  // pumping messages, the test-explorer poller, live panel effects. A fixed
  // sleep encodes "the host is idle", which is true only when this file runs
  // alone; in full-suite order the reply lands late and the driver samples a
  // DOM that has not reacted yet. Returns false on timeout so the caller's
  // assertion still fails loudly rather than the whole scenario hanging.
  const waitUntil = async (predicate, tries) => {
    for (let attempt = 0; attempt < (tries || 400); attempt += 1) {
      try { if (predicate()) return true; } catch (ignored) { /* not rendered yet */ }
      await sleep(25);
    }
    return false;
  };
  const dialog = () => document.getElementById('preview-dialog');
  // Read every observable fact about the Project view's setting panels at
  // this instant — Typeshed and Caching.
  const probe = () => {
    const commit = el('[data-typeshed-commit]');
    const commitError = document.getElementById('typeshed-commit-error');
    const pkg = el('[data-typeshed-package]');
    const packageError = document.getElementById('typeshed-package-error');
    const path = el('[data-typeshed-path="TypeshedPath"]');
    const storeFolder = el('[data-typeshed-path="TypeshedStorePath"]');
    const pickFolder = el('[data-pick-typeshed-folder="TypeshedPath"]');
    const noSource = el('.typeshed-no-source');
    const status = {};
    const rows = all('#typeshed-status dt');
    rows.forEach((dt, index) => { status[text(dt)] = text(all('#typeshed-status dd')[index]); });
    return {
      sources: all('[data-typeshed-source]').map((radio) => ({
        mode: radio.dataset.typeshedSource,
        checked: radio.checked,
        disabled: radio.disabled,
        hint: text(radio.parentElement.querySelector('small')),
      })),
      commitPresent: commit !== null,
      commitValue: commit ? commit.value : null,
      commitDisabled: commit ? commit.disabled : null,
      commitInvalid: commit ? commit.getAttribute('aria-invalid') : null,
      commitError: commitError && !commitError.hidden ? text(commitError) : null,
      packagePresent: pkg !== null,
      packageValue: pkg ? pkg.value : null,
      packageInvalid: pkg ? pkg.getAttribute('aria-invalid') : null,
      packageError: packageError && !packageError.hidden ? text(packageError) : null,
      pathPresent: path !== null,
      pathValue: path ? path.value : null,
      pickFolderDisabled: pickFolder ? pickFolder.disabled : null,
      storePickerDisabled: el('[data-pick-typeshed-folder="TypeshedStorePath"]')
        ? el('[data-pick-typeshed-folder="TypeshedStorePath"]').disabled : null,
      textControls: all('[data-typeshed-text]').length,
      advancedPresent: el('.typeshed-advanced') !== null,
      advancedOpen: el('.typeshed-advanced') ? el('.typeshed-advanced').open : null,
      storeFolderValue: storeFolder ? storeFolder.value : null,
      booleanControls: all('[data-typeshed-boolean]').length,
      actions: all('[data-typeshed-action]').map((button) => ({
        action: button.dataset.typeshedAction,
        label: text(button),
        disabled: button.disabled,
        busy: button.classList.contains('busy'),
      })),
      status,
      warnings: all('.typeshed-warning').map(text),
      noSourcePresent: noSource !== null,
      noSourceText: text(noSource),
      // The caching panel ([LSPCFGED-CACHE]): the persistent cache's two
      // controls, plus the read-only in-session rows that keep the Salsa
      // layer from looking like an omission in the config file.
      cacheEnabledPresent: el('[data-cache-enabled]') !== null,
      cacheEnabled: el('[data-cache-enabled]') ? el('[data-cache-enabled]').checked : null,
      cacheFolderValue: el('[data-cache-folder]') ? el('[data-cache-folder]').value : null,
      cacheResetPresent: el('[data-action="reset-cache-folder"]') !== null,
      cachePickerDisabled: el('[data-pick-cache-folder]')
        ? el('[data-pick-cache-folder]').disabled : null,
      inSession: (() => {
        const rows = {};
        all('#cache-in-session dt').forEach((dt, index) => {
          rows[text(dt)] = text(all('#cache-in-session dd')[index]);
        });
        return rows;
      })(),
      // The deleted lock screen must stay deleted: no overlay node, no inert
      // shell, ever ([LSPCFGED-TYPESHED-DOWNLOAD]).
      overlayPresent: document.getElementById('state-overlay') !== null,
      shellInert: document.getElementById('shell').inert === true,
      dialogOpen: document.getElementById('preview-dialog').open,
      dialogChanges: text(document.getElementById('preview-changes')),
    };
  };
  const steps = [];
  const record = (label) => { steps.push(Object.assign({ label }, probe())); return steps[steps.length - 1]; };
  const click = async (node) => { node.click(); await sleep(settleDelay); };
  const change = async (node, value) => {
    if (typeof value === 'boolean') { node.checked = value; } else { node.value = value; }
    node.dispatchEvent(new Event('change', { bubbles: true }));
    await sleep(settleDelay);
  };
  // A real click: a disabled radio does nothing, exactly as for a user.
  const chooseSource = async (mode) => {
    el('[data-typeshed-source="' + mode + '"]').click();
    await sleep(settleDelay);
  };
`;
