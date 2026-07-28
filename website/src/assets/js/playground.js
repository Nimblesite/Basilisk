// Implements [WASM-PLAN-SITE]: Monaco, local WASM diagnostics, and hash sharing.
const samples = {
  generics: `def answer() -> int:\n    return "not an int"\n\nprint(answer())`,
  protocol: `from typing import Protocol\n\nclass Renderable(Protocol):\n    def render(self) -> str: ...\n\nclass User:\n    name: str = "Ada"\n\ndef show(item: Renderable) -> None:\n    print(item.render())\n\nshow(User())`,
  clean: `from dataclasses import dataclass\n\n@dataclass\nclass User:\n    name: str\n    active: bool = True\n\ndef greeting(user: User) -> str:\n    return f"Hello, {user.name}!"\n\nprint(greeting(User("Ada")))`,
};
const byId = (id) => document.getElementById(id);
const ui = { button: byId("check-code"), count: byId("diagnostic-count"), list: byId("diagnostics"), status: byId("engine-status"), timing: byId("check-timing"), version: byId("python-version") };
let editor;
let enginePromise;

function sourceFromHash() {
  const encoded = new URLSearchParams(location.hash.slice(1)).get("code");
  if (!encoded) return samples.generics;
  try { return LZString.decompressFromEncodedURIComponent(encoded) || samples.generics; } catch { return samples.generics; }
}

const loadEngine = () => enginePromise ||= import("/assets/wasm/basilisk_wasm.js").then(async (module) => { await module.default(); return module; });
const marker = (item) => ({ severity: monaco.MarkerSeverity.Error, message: item.message, code: item.code || undefined, startLineNumber: item.line, startColumn: item.col, endLineNumber: item.end_line, endColumn: Math.max(item.end_col, item.col + 1), source: "Basilisk" });

function renderDiagnostics(diagnostics) {
  monaco.editor.setModelMarkers(editor.getModel(), "basilisk", diagnostics.map(marker));
  ui.count.textContent = String(diagnostics.length);
  ui.count.classList.toggle("is-clean", diagnostics.length === 0);
  if (!diagnostics.length) {
    ui.list.innerHTML = `<li class="diagnostics-empty diagnostics-empty--clean"><div class="empty-glyph">✓</div><strong>Looking sharp</strong><p>No type errors found in this file.</p></li>`;
    return;
  }
  ui.list.replaceChildren(...diagnostics.map((diagnostic) => {
    const item = document.createElement("li");
    item.innerHTML = `<button type="button"><span class="diagnostic-location">${diagnostic.line}:${diagnostic.col}</span><span class="diagnostic-message"></span></button>`;
    item.querySelector(".diagnostic-message").textContent = diagnostic.message;
    if (diagnostic.code) {
      const link = document.createElement("a");
      link.href = `/errors/${encodeURIComponent(diagnostic.code)}/`;
      link.textContent = diagnostic.code;
      link.title = `Read about ${diagnostic.code}`;
      item.prepend(link);
    }
    item.querySelector("button").addEventListener("click", () => { editor.setPosition({ lineNumber: diagnostic.line, column: diagnostic.col }); editor.focus(); });
    return item;
  }));
}

async function checkCode() {
  const started = performance.now();
  ui.button.disabled = true;
  ui.status.className = "engine-status is-loading";
  ui.status.innerHTML = "<span></span>Analysing locally…";
  try {
    const engine = await loadEngine();
    const options = ui.version.value ? { python_version: ui.version.value } : {};
    renderDiagnostics(JSON.parse(engine.check(editor.getValue(), JSON.stringify(options))).diagnostics);
    ui.timing.textContent = `Checked in ${Math.round(performance.now() - started)} ms`;
    ui.status.className = "engine-status is-ready";
    ui.status.innerHTML = "<span></span>Engine ready · running locally";
  } catch (error) {
    ui.status.className = "engine-status is-error";
    ui.status.textContent = "Engine failed to load";
    ui.list.innerHTML = `<li class="diagnostics-empty"><strong>Could not start Basilisk</strong><p></p></li>`;
    ui.list.querySelector("p").textContent = String(error.message || error);
  } finally { ui.button.disabled = false; }
}

window.require.config({ paths: { vs: "/assets/vendor/monaco/vs" } });
window.require(["vs/editor/editor.main"], () => {
  editor = monaco.editor.create(byId("editor"), { value: sourceFromHash(), language: "python", automaticLayout: true, minimap: { enabled: false }, fontFamily: "'SFMono-Regular', Consolas, monospace", fontSize: 14, lineHeight: 23, padding: { top: 18, bottom: 18 }, scrollBeyondLastLine: false, renderLineHighlight: "all", theme: document.documentElement.dataset.theme === "dark" ? "vs-dark" : "vs" });
  byId("editor-loading").remove();
  editor.addAction({ id: "basilisk.check", label: "Check with Basilisk", keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter], run: checkCode });
  editor.onDidChangeCursorPosition(({ position }) => { byId("cursor-line").textContent = position.lineNumber; byId("cursor-col").textContent = position.column; });
  ui.button.addEventListener("click", checkCode);
  loadEngine().then(() => { ui.status.className = "engine-status is-ready"; ui.status.innerHTML = "<span></span>Engine ready · running locally"; });
});

byId("share-code").addEventListener("click", async (event) => {
  const url = `${location.origin}${location.pathname}#code=${LZString.compressToEncodedURIComponent(editor.getValue())}`;
  await navigator.clipboard.writeText(url);
  event.currentTarget.textContent = "Copied!";
  setTimeout(() => { event.currentTarget.textContent = "Share"; }, 1400);
});
document.querySelectorAll("[data-example]").forEach((button) => button.addEventListener("click", () => { editor.setValue(samples[button.dataset.example]); editor.focus(); checkCode(); }));
