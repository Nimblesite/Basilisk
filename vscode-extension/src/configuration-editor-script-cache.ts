// Implements [LSPCFGED-CACHE] caching controls for the Project view.
/** Caching fragment of the dependency-free webview runtime. */

export const CONFIGURATION_EDITOR_SCRIPT_CACHE = String.raw`
    // Basilisk caches on two layers and this panel names BOTH ([LSPCFGED-CACHE]).
    // Only one of them is configuration; saying so is the whole point of the
    // panel, because a surface that shows a single "cache" switch reads as if
    // that switch were all the caching there is.
    function cacheState() { return snapshot.cache; }
    function persistentCache() { return cacheState().persistent; }

    function cacheKey(name) { return { kind: name }; }
    function cacheSet(name, value) {
      return { kind: 'SetCacheSetting', key: cacheKey(name), value };
    }
    function cacheRemove(name) {
      return { kind: 'RemoveCacheSetting', key: cacheKey(name) };
    }

    // The toggle always writes an explicit 'cache = true|false', exactly as a
    // severity dropdown always writes an explicit entry: what the panel shows
    // is then what the file says, with no inferred middle state.
    function cacheEnabledField() {
      const field = document.createElement('label');
      field.className = 'cache-toggle';
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.checked = persistentCache().enabled;
      input.dataset.cacheEnabled = 'CacheEnabled';
      field.append(
        input,
        textNode('span', 'Reuse results between runs'),
        textNode('small', 'Writes the cache setting to pyproject.toml; entries live in the cache folder below. Applies to basilisk check/analyze runs and to the editor\'s startup scan of the workspace. A cached result is replayed only when the file, everything it imports, the configuration, the typeshed source, and the Basilisk version are all unchanged — otherwise the file is checked in full.'),
      );
      return field;
    }

    // The server sends the folder the next run actually resolves, default
    // included, so the panel can show a location without the project having
    // chosen one. Reset only exists once there IS a choice to undo.
    function cacheFolderField() {
      const cache = persistentCache();
      const field = document.createElement('label');
      field.className = 'cache-field';
      field.append(
        textNode('span', 'Cache folder'),
        textNode('small', cache.folderConfigured
          ? 'Set by cache-dir in pyproject.toml.'
          : 'Default location. Basilisk has not been told to use another.'),
      );
      const picker = document.createElement('div');
      picker.className = 'path-picker';
      const input = document.createElement('input');
      input.type = 'text';
      input.readOnly = true;
      input.value = cache.folder || '';
      input.dataset.cacheFolder = 'CacheDir';
      const choose = textNode('button', 'Change…', 'secondary');
      choose.type = 'button';
      choose.dataset.pickCacheFolder = 'CacheDir';
      picker.append(input, choose);
      field.append(picker);
      if (cache.folderConfigured) {
        const reset = textNode('button', 'Use default folder', 'secondary');
        reset.type = 'button';
        reset.dataset.action = 'reset-cache-folder';
        field.append(reset);
      }
      return field;
    }

    // Read-only, and deliberately so: the in-session engine has no key to
    // offer ([CHKARCH-INCREMENTAL-SALSA]). Stating that here is what stops its
    // absence from the config file reading as an omission.
    function inSessionRows() {
      const tracked = cacheState().inSession.trackedFiles;
      return [
        ['Engine', 'Salsa incremental queries'],
        ['State', 'Always on · no configuration'],
        ['Memoized files', formatNumber(tracked) + ' tracked in this session'],
      ];
    }
    function renderInSessionCache() {
      const target = byId('cache-in-session');
      clear(target);
      target.append(textNode('p', 'Editing is incremental on its own. parse → resolve → check is one memoized query per file, so an edit re-runs only the file you touched and the files that import it. It lives in memory for the life of the session, needs no setting, and cannot be switched off.'));
      const summary = document.createElement('dl');
      inSessionRows().forEach((row) => summary.append(textNode('dt', row[0]), textNode('dd', row[1])));
      target.append(summary);
    }

    // A cache write re-renders this section from the fresh snapshot; the
    // rebuild must not eat the user's focus.
    function cacheFocusSelector(active) {
      if (!active || !active.dataset) return undefined;
      if (active.dataset.cacheEnabled) return '[data-cache-enabled]';
      if (active.dataset.pickCacheFolder) return '[data-pick-cache-folder]';
      return undefined;
    }
    function renderCacheControls() {
      const active = document.activeElement;
      const selector = cacheFocusSelector(active);
      const controls = byId('cache-controls');
      clear(controls);
      controls.append(cacheEnabledField(), cacheFolderField());
      renderInSessionCache();
      if (!selector) return;
      const restored = document.querySelector(selector);
      if (restored) restored.focus({ preventScroll: true });
    }

    /** Every caching control change, routed from the one delegated listener. */
    function cacheChanged(target) {
      if (!target.dataset.cacheEnabled) return false;
      postPreview([cacheSet('CacheEnabled', target.checked ? 'true' : 'false')]);
      announce(target.checked ? 'Persistent result cache enabled' : 'Persistent result cache disabled');
      return true;
    }
`;
