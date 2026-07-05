// Implements [PROFILE-WEBVIEW-HOST]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-WEBVIEW-HOST
/**
 * Shared webview host for every profiler results panel (flamegraph, memory
 * dashboard, retention graph).
 *
 * All three panels share the same lifecycle needs — a singleton panel that is
 * created once and re-revealed after, a message handler that must be registered
 * exactly ONCE per panel instance, a nonce-gated CSP, and safe embedding of
 * profiled-program data into inline scripts. Before this module each panel
 * hand-rolled that stack and two of them re-registered their message handler on
 * every open (so after N snapshots one click navigated N times) and shipped no
 * CSP at all. Hosting the primitives once makes those bugs unrepresentable.
 */

import { randomBytes } from "node:crypto";
import * as vscode from "vscode";

// ── Safe script embedding ─────────────────────────────────────────────────

/** Entropy (bytes) for the per-render webview CSP nonce. */
const CSP_NONCE_BYTES = 16;

/**
 * Serialize a value for embedding inside an inline `<script>`. `JSON.stringify`
 * does not escape `<`, so profiled-program data containing `</script>` would
 * close the script element early; escaping `<` keeps it an opaque JS string.
 */
export function embedJson(value: unknown): string {
  return JSON.stringify(value).split("<").join("\\u003c");
}

// ── Document shell ────────────────────────────────────────────────────────

/** The parts every profiler webview document is assembled from. */
export interface WebviewDocument {
  readonly title: string;
  readonly css: string;
  readonly body: string;
  readonly script: string;
}

/**
 * Build a complete, CSP-locked webview HTML document. A fresh nonce gates the
 * (self-generated) inline script; `default-src 'none'` blocks every external
 * resource; `img-src data:` admits only inline data URIs (the embedded flame
 * graph SVG). Even if profiled-program data slipped an escape, the browser
 * refuses to run any inline script without the nonce.
 */
export function buildWebviewDocument(doc: WebviewDocument): string {
  const nonce = randomBytes(CSP_NONCE_BYTES).toString("base64");
  const csp = `default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="${csp}">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${doc.title}</title>
  <style>${doc.css}</style>
</head>
<body>${doc.body}
  <script nonce="${nonce}">${doc.script}</script>
</body>
</html>`;
}

// ── Singleton panel ───────────────────────────────────────────────────────

/** A message posted from a profiler webview back to the extension. */
export interface WebviewMessage {
  readonly type: string;
  readonly file?: string;
  readonly line?: number;
}

/**
 * Route the message every profiler panel shares: a click on a source location
 * opens that file beside the panel. Returns whether the message was handled so
 * panels can layer their own message types on top.
 */
export function handleSourceNavigation(msg: WebviewMessage): boolean {
  if (msg.type !== "navigateToSource" || msg.file === undefined || msg.line === undefined) {
    return false;
  }
  const position = new vscode.Position(msg.line - 1, 0);
  void vscode.window.showTextDocument(vscode.Uri.file(msg.file), {
    selection: new vscode.Range(position, position),
    viewColumn: vscode.ViewColumn.One,
  });
  return true;
}

/**
 * A create-once / reveal-after webview panel whose message handler is bound
 * exactly once per panel instance — re-opening with fresh data re-renders the
 * HTML but never stacks a second handler.
 */
export class SingletonWebviewPanel {
  private panel: vscode.WebviewPanel | undefined;

  constructor(
    private readonly viewType: string,
    private readonly onMessage: (msg: WebviewMessage) => void,
  ) {}

  /** Open the panel (or reveal the existing one) and swap in the new document. */
  public show(title: string, html: string): void {
    if (this.panel !== undefined) {
      this.panel.title = title;
      this.panel.reveal(vscode.ViewColumn.Beside);
    } else {
      this.panel = vscode.window.createWebviewPanel(
        this.viewType,
        title,
        vscode.ViewColumn.Beside,
        { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] },
      );
      this.panel.onDidDispose(() => {
        this.panel = undefined;
      });
      // Bound once per panel instance — the whole reason this class exists.
      this.panel.webview.onDidReceiveMessage(this.onMessage);
    }
    this.panel.webview.html = html;
  }

  /** Whether the panel is currently open (e2e seam). */
  public isOpen(): boolean {
    return this.panel !== undefined;
  }

  /** Close and forget the panel (extension teardown). */
  public dispose(): void {
    this.panel?.dispose();
    this.panel = undefined;
  }
}
