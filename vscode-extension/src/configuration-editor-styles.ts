// Implements [VSIX-CONFIGURATION-EDITOR-HOST].
/** Theme-native visual system for the configuration editor webview. */

export const CONFIGURATION_EDITOR_STYLES = `
  :root {
    color-scheme: light dark;
    --bsk-orange: #e65305;
    --bsk-orange-soft: color-mix(in srgb, var(--bsk-orange) 14%, transparent);
    --bsk-sky: #3aa3d3;
    --bsk-sky-soft: color-mix(in srgb, var(--bsk-sky) 14%, transparent);
    --bg: var(--vscode-editor-background);
    --surface: var(--vscode-sideBar-background, var(--vscode-editor-background));
    --surface-raised: var(--vscode-editorWidget-background, var(--surface));
    --border: var(--vscode-panel-border, var(--vscode-widget-border, transparent));
    --text: var(--vscode-editor-foreground);
    --muted: var(--vscode-descriptionForeground);
    --focus: var(--vscode-focusBorder);
    --error: var(--vscode-errorForeground);
    --warning: var(--vscode-editorWarning-foreground, #cca700);
    --info: var(--vscode-editorInfo-foreground, var(--bsk-sky));
    --disabled: var(--vscode-disabledForeground);
    --radius: 10px;
    --rule-height: 112px;
  }

  * { box-sizing: border-box; }
  html, body { height: 100%; }
  body {
    margin: 0;
    overflow: hidden;
    background: var(--bg);
    color: var(--text);
    font: var(--vscode-font-size) / 1.45 var(--vscode-font-family);
  }
  button, input, select { font: inherit; }
  button, select, input[type="search"], input[type="text"] {
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  button { cursor: pointer; }
  button:focus-visible, input:focus-visible, select:focus-visible, [tabindex]:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  button:disabled, select:disabled { cursor: not-allowed; opacity: .55; }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  #skip-link {
    position: fixed;
    z-index: 20;
    top: 8px;
    left: 8px;
    padding: 7px 10px;
    transform: translateY(-150%);
    background: var(--vscode-button-background);
    color: var(--vscode-button-foreground);
  }
  #skip-link:focus { transform: translateY(0); }

  body > header {
    position: sticky;
    z-index: 8;
    top: 0;
    min-height: 74px;
    display: grid;
    grid-template-columns: minmax(250px, 1fr) minmax(180px, auto) auto;
    align-items: center;
    gap: 18px;
    padding: 12px 20px;
    background: color-mix(in srgb, var(--bg) 94%, transparent);
    border-bottom: 1px solid var(--border);
    backdrop-filter: blur(14px);
  }
  #identity { display: flex; align-items: center; min-width: 0; gap: 12px; }
  #mark {
    width: 38px;
    height: 38px;
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    border: 1px solid color-mix(in srgb, var(--bsk-orange) 55%, var(--border));
    border-radius: 12px 5px 12px 5px;
    background: linear-gradient(145deg, var(--bsk-orange-soft), var(--bsk-sky-soft));
    color: var(--bsk-orange);
    font-weight: 800;
    letter-spacing: -.08em;
  }
  h1 { margin: 0; font-size: 17px; line-height: 1.2; letter-spacing: -.01em; }
  #root-label, #source-label {
    overflow: hidden;
    color: var(--muted);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  #source-block { min-width: 0; text-align: right; }
  #source-label { display: block; max-width: 38vw; }
  #header-actions { display: flex; align-items: center; justify-content: flex-end; gap: 8px; }
  #status-pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    max-width: 230px;
    padding: 5px 9px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--muted);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  #status-pill::before {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--bsk-sky);
    content: "";
  }
  #status-pill[data-phase="error"]::before,
  #status-pill[data-phase="conflict"]::before { background: var(--error); }
  #status-pill[data-phase="applying"]::before,
  #status-pill[data-phase="previewing"]::before,
  #status-pill[data-phase="loading"]::before { animation: breathe 1.2s ease-in-out infinite; }

  .icon-button, .secondary, .primary, .danger {
    min-height: 30px;
    padding: 5px 10px;
  }
  .icon-button, .secondary {
    background: var(--vscode-button-secondaryBackground);
    color: var(--vscode-button-secondaryForeground);
  }
  .icon-button:hover, .secondary:hover { background: var(--vscode-button-secondaryHoverBackground); }
  .primary { background: var(--vscode-button-background); color: var(--vscode-button-foreground); }
  .primary:hover { background: var(--vscode-button-hoverBackground); }
  .danger { background: var(--error); color: var(--vscode-button-foreground); }

  #shell {
    height: calc(100vh - 74px);
    display: grid;
    grid-template-columns: 178px minmax(0, 1fr);
  }
  #section-nav {
    padding: 18px 10px;
    overflow-y: auto;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }
  #section-nav button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    margin-bottom: 4px;
    padding: 8px 10px;
    background: transparent;
    border-color: transparent;
    color: var(--muted);
    text-align: left;
  }
  #section-nav button:hover { background: var(--vscode-list-hoverBackground); color: var(--text); }
  #section-nav button[aria-current="page"] {
    background: var(--vscode-list-activeSelectionBackground);
    color: var(--vscode-list-activeSelectionForeground);
    font-weight: 600;
  }
  #section-nav button span:first-child { width: 18px; text-align: center; }

  main { min-width: 0; overflow: hidden; }
  main > section { height: 100%; overflow: auto; padding: 22px; }
  main > section[hidden] { display: none; }
  .section-heading { max-width: 920px; margin: 0 auto 20px; }
  .section-heading h2 { margin: 0 0 4px; font-size: 22px; letter-spacing: -.02em; }
  .section-heading p { margin: 0; color: var(--muted); }

  #overview-grid, #adoption-grid, #project-grid {
    width: min(100%, 920px);
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
    margin: 0 auto;
  }
  .card {
    position: relative;
    min-width: 0;
    padding: 16px;
    background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 1px 0 color-mix(in srgb, var(--text) 5%, transparent);
  }
  .card[data-accent]::before {
    position: absolute;
    top: -1px;
    left: 14px;
    width: 38px;
    height: 2px;
    border-radius: 2px;
    background: var(--bsk-orange);
    content: "";
  }
  .card[data-accent="sky"]::before { background: var(--bsk-sky); }
  .card h3 { margin: 0 0 5px; font-size: 13px; }
  .card p { margin: 0 0 12px; color: var(--muted); font-size: 12px; }
  .stat { display: block; margin-top: 4px; font-size: 28px; font-weight: 650; letter-spacing: -.04em; }
  #severity-strip { grid-column: 1 / -1; display: grid; grid-template-columns: repeat(5, 1fr); gap: 1px; padding: 0; overflow: hidden; }
  #severity-strip div { padding: 14px; background: var(--surface-raised); }
  #severity-strip strong { display: block; font-size: 20px; }
  #severity-strip span { color: var(--muted); font-size: 11px; }
  .wide { grid-column: 1 / -1; }
  .action-row { display: flex; flex-wrap: wrap; gap: 8px; }
  #preset-list, #adoption-presets { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); }
  .preset-card {
    min-width: 0;
    display: grid;
    grid-template-rows: auto 1fr auto;
    gap: 4px;
    padding: 11px 12px;
    background: linear-gradient(145deg, var(--surface), var(--surface-raised));
    border-color: color-mix(in srgb, var(--bsk-orange) 38%, var(--border));
    color: var(--text);
    text-align: left;
  }
  .preset-card:hover { background: var(--bsk-orange-soft); border-color: var(--bsk-orange); }
  .preset-card strong { font-size: 13px; }
  .preset-card span { color: var(--muted); font-size: 11px; }
  .preset-card small { color: var(--bsk-orange); font-size: 10px; font-weight: 600; }

  #rules-section { padding: 0; overflow: hidden; }
  #rules-layout { height: 100%; display: grid; grid-template-columns: 190px minmax(360px, 1fr) minmax(240px, 320px); }
  #tag-rail, #rule-detail { min-width: 0; padding: 18px 12px; overflow-y: auto; background: var(--surface); }
  #tag-rail { border-right: 1px solid var(--border); }
  #rule-detail { border-left: 1px solid var(--border); }
  #tag-rail h2, #rule-detail h2 { margin: 0 8px 10px; font-size: 12px; text-transform: uppercase; letter-spacing: .08em; }
  #tag-list { display: grid; gap: 14px; }
  .tag-group { display: grid; gap: 3px; }
  .tag-group h3 { margin: 0 8px 2px; color: var(--muted); font-size: 10px; font-weight: 600; letter-spacing: .06em; text-transform: uppercase; }
  .tag-button {
    width: 100%;
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
    padding: 7px 8px;
    background: transparent;
    border-color: transparent;
    color: var(--text);
    text-align: left;
  }
  .tag-button:hover { background: var(--vscode-list-hoverBackground); }
  .tag-button[aria-pressed="true"] { background: var(--bsk-sky-soft); border-color: var(--bsk-sky); }
  .tag-button small { color: var(--muted); }
  #rules-workspace { min-width: 0; display: grid; grid-template-rows: auto 1fr; overflow: hidden; }
  #rules-toolbar { display: flex; align-items: center; gap: 8px; padding: 12px; border-bottom: 1px solid var(--border); }
  #rule-search { width: 100%; min-width: 120px; padding: 7px 10px; background: var(--vscode-input-background); color: var(--vscode-input-foreground); border-color: var(--vscode-input-border, var(--border)); }
  #filter-result { flex: 0 0 auto; color: var(--muted); font-size: 11px; }
  #rule-viewport { position: relative; overflow: auto; contain: strict; }
  #rule-spacer { position: relative; width: 100%; }
  #rule-window { position: absolute; inset: 0 0 auto; }
  .rule-row {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--rule-height);
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  .rule-row:hover { background: var(--vscode-list-hoverBackground); }
  .rule-row[data-selected="true"] { background: var(--bsk-sky-soft); }
  .rule-copy { min-width: 0; }
  .rule-copy button { max-width: 100%; padding: 0; background: transparent; border: 0; color: var(--text); text-align: left; }
  .rule-copy strong { font: 600 12px var(--vscode-editor-font-family); }
  .rule-copy .title { margin-left: 7px; font-weight: 600; }
  .rule-copy p { margin: 4px 0; overflow: hidden; color: var(--muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .chip-list { display: flex; gap: 4px; overflow: hidden; }
  .chip { padding: 1px 6px; border: 1px solid var(--border); border-radius: 999px; color: var(--muted); font-size: 10px; white-space: nowrap; }
  .metrics { margin-left: 5px; color: var(--muted); font-size: 10px; white-space: nowrap; }
  .severity-select { min-width: 108px; padding: 6px; background: var(--vscode-dropdown-background); color: var(--vscode-dropdown-foreground); border-color: var(--vscode-dropdown-border, var(--border)); }
  .severity-select[data-severity="Error"] { border-left: 3px solid var(--error); }
  .severity-select[data-severity="Warning"] { border-left: 3px solid var(--warning); }
  .severity-select[data-severity="Info"] { border-left: 3px solid var(--info); }
  .severity-select[data-severity="Disabled"] { border-left: 3px solid var(--disabled); }
  #detail-empty, .empty-state { padding: 24px 10px; color: var(--muted); text-align: center; }
  #detail-content dl { display: grid; grid-template-columns: auto 1fr; gap: 6px 10px; }
  #detail-content dt { color: var(--muted); }
  #detail-content dd { margin: 0; overflow-wrap: anywhere; }
  #occurrence-list { display: grid; gap: 6px; margin-top: 12px; }
  .occurrence { padding: 7px; background: transparent; color: var(--text); text-align: left; }
  .occurrence small { display: block; color: var(--muted); }

  #bulk-tray {
    position: fixed;
    z-index: 12;
    right: 22px;
    bottom: 18px;
    left: 210px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: var(--vscode-editorWidget-background);
    border: 1px solid var(--focus);
    border-radius: 9px;
    box-shadow: 0 8px 30px var(--vscode-widget-shadow);
  }
  #bulk-tray[hidden] { display: none; }
  #selection-count { margin-right: auto; font-weight: 600; }
  #bulk-setting, #path-setting { padding: 6px; background: var(--vscode-dropdown-background); color: var(--vscode-dropdown-foreground); }

  #path-override-list { display: grid; gap: 9px; }
  .path-override-card {
    display: grid;
    grid-template-columns: minmax(140px, 1fr) auto auto;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .path-override-card h3 { margin: 0; overflow-wrap: anywhere; font: 600 12px var(--vscode-editor-font-family); }
  .path-override-card ul { grid-column: 1 / -1; display: grid; gap: 4px; margin: 0; padding: 0; list-style: none; }
  .path-override-card li { display: flex; justify-content: space-between; gap: 12px; color: var(--muted); }
  .path-override-card code { color: var(--text); overflow-wrap: anywhere; }

  dialog {
    width: min(620px, calc(100vw - 36px));
    max-height: calc(100vh - 32px);
    padding: 0;
    background: var(--vscode-editorWidget-background);
    color: var(--text);
    border: 1px solid var(--focus);
    border-radius: 12px;
    box-shadow: 0 14px 50px var(--vscode-widget-shadow);
  }
  dialog[open] { display: grid; grid-template-rows: auto minmax(0, 1fr) auto; }
  dialog::backdrop { background: color-mix(in srgb, #000 42%, transparent); }
  dialog header, dialog footer { padding: 15px 18px; border-bottom: 1px solid var(--border); }
  dialog footer { display: flex; justify-content: flex-end; gap: 8px; border-top: 1px solid var(--border); border-bottom: 0; }
  dialog h2 { margin: 0; font-size: 17px; }
  #preview-body { min-height: 0; padding: 18px; overflow-y: auto; }
  #impact-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
  #impact-grid div { padding: 10px; background: var(--surface); border: 1px solid var(--border); border-radius: 7px; }
  #impact-grid strong { display: block; font-size: 19px; }
  #impact-grid span { color: var(--muted); font-size: 10px; }
  #preview-changes { display: grid; gap: 4px; }
  .preview-change {
    display: grid;
    grid-template-columns: minmax(110px, 1fr) minmax(100px, 1fr) auto;
    gap: 10px;
    padding: 7px 9px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 5px;
  }
  .preview-change code { overflow-wrap: anywhere; }
  .preview-change span { color: var(--muted); overflow-wrap: anywhere; }
  .preview-change strong { color: var(--bsk-orange); font-size: 11px; text-align: right; }

  #state-overlay {
    position: fixed;
    z-index: 30;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 24px;
    background: color-mix(in srgb, var(--bg) 94%, transparent);
  }
  #state-overlay[hidden] { display: none; }
  #state-card { width: min(480px, 100%); padding: 26px; text-align: center; }
  #state-symbol { width: 44px; height: 44px; display: grid; place-items: center; margin: 0 auto 12px; border-radius: 50%; background: var(--bsk-orange-soft); color: var(--bsk-orange); font-size: 20px; }
  #state-card h2 { margin: 0 0 5px; }
  #state-card p { margin: 0 0 14px; color: var(--muted); }
  .overlay-actions { justify-content: center; }
  #problem-banner { max-width: 920px; margin: 0 auto 14px; padding: 10px 12px; background: color-mix(in srgb, var(--error) 10%, transparent); border: 1px solid var(--error); border-radius: 7px; }
  #problem-banner[hidden] { display: none; }

  @keyframes breathe { 50% { opacity: .25; transform: scale(.75); } }
  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after { scroll-behavior: auto !important; transition: none !important; animation: none !important; }
  }
  @media (max-width: 980px) {
    #rules-section { overflow: auto; }
    #rules-layout {
      height: auto;
      min-height: 100%;
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto minmax(480px, 70vh) auto;
    }
    #tag-rail { overflow: visible; border-right: 0; border-bottom: 1px solid var(--border); }
    #tag-list { grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); }
    #rule-detail { display: block; overflow: visible; border-top: 1px solid var(--border); border-left: 0; }
    body > header { grid-template-columns: minmax(220px, 1fr) auto; }
    #source-block { display: none; }
  }
  @media (max-width: 720px) {
    body > header { min-height: 64px; padding: 9px 12px; }
    #status-pill { display: none; }
    #shell { height: calc(100vh - 64px); grid-template-columns: minmax(0, 1fr); grid-template-rows: auto minmax(0, 1fr); }
    #section-nav { display: flex; gap: 4px; padding: 7px; overflow-x: auto; border-right: 0; border-bottom: 1px solid var(--border); }
    #section-nav button { width: auto; flex: 0 0 auto; justify-content: center; margin: 0; padding: 8px; }
    #rules-layout { grid-template-columns: minmax(0, 1fr); }
    #tag-rail { display: block; }
    #bulk-tray { left: 12px; right: 12px; bottom: 10px; flex-wrap: wrap; }
    .path-override-card, .preview-change { grid-template-columns: minmax(0, 1fr); }
    .preview-change strong { text-align: left; }
    main > section { padding: 16px; }
  }
  body.vscode-high-contrast, body.vscode-high-contrast-light {
    --border: var(--vscode-contrastBorder);
    --bsk-orange: var(--vscode-foreground);
    --bsk-sky: var(--vscode-focusBorder);
  }
`;
