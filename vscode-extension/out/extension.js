"use strict";
/**
 * Basilisk VS Code Extension
 *
 * Runs `basilisk check --output json` on every Python file save/open and
 * pushes the resulting diagnostics into VSCode's Problems panel.
 *
 * NOTE: This extension uses the subprocess approach (no LSP).
 * LSP integration is deferred to a future phase — see docs/lsp-plan.md.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const child_process_1 = require("child_process");
const path = __importStar(require("path"));
const COLLECTION_NAME = "basilisk";
function activate(context) {
    const collection = vscode.languages.createDiagnosticCollection(COLLECTION_NAME);
    context.subscriptions.push(collection);
    // Check on open.
    context.subscriptions.push(vscode.workspace.onDidOpenTextDocument((doc) => {
        if (doc.languageId === "python") {
            checkDocument(doc, collection);
        }
    }));
    // Check on save.
    context.subscriptions.push(vscode.workspace.onDidSaveTextDocument((doc) => {
        if (doc.languageId === "python") {
            checkDocument(doc, collection);
        }
    }));
    // Clear diagnostics when a file is closed.
    context.subscriptions.push(vscode.workspace.onDidCloseTextDocument((doc) => {
        collection.delete(doc.uri);
    }));
    // Check all already-open Python documents on activation.
    for (const doc of vscode.workspace.textDocuments) {
        if (doc.languageId === "python") {
            checkDocument(doc, collection);
        }
    }
}
function deactivate() {
    // Nothing to tear down; the DiagnosticCollection is disposed via subscriptions.
}
function getConfig() {
    const cfg = vscode.workspace.getConfiguration("basilisk");
    return {
        executablePath: cfg.get("executablePath") ?? "basilisk",
        enabled: cfg.get("enabled") ?? true,
    };
}
function checkDocument(doc, collection) {
    const { executablePath, enabled } = getConfig();
    if (!enabled) {
        collection.delete(doc.uri);
        return;
    }
    // Only check files on disk — unsaved buffers have no path the binary can read.
    if (doc.isUntitled || doc.uri.scheme !== "file") {
        return;
    }
    const filePath = doc.uri.fsPath;
    (0, child_process_1.execFile)(executablePath, ["check", "--output", "json", filePath], { cwd: workspaceRoot() }, (error, stdout, stderr) => {
        // Exit code 1 means diagnostics found — that's normal, not a crash.
        // Exit code 3 means internal error.
        if (error && error.code === 3) {
            vscode.window.showWarningMessage(`Basilisk: internal error checking ${path.basename(filePath)}: ${stderr}`);
            return;
        }
        // Any other non-zero exit (e.g. binary not found) should also surface.
        if (error && typeof error.code === "number" && error.code !== 1) {
            vscode.window.showWarningMessage(`Basilisk: failed to run '${executablePath}'. ` +
                `Is it installed and on PATH? (${error.message})`);
            collection.delete(doc.uri);
            return;
        }
        const diagnostics = parseDiagnostics(stdout, doc);
        collection.set(doc.uri, diagnostics);
    });
}
function parseDiagnostics(json, doc) {
    let items;
    try {
        items = JSON.parse(json);
    }
    catch {
        // Malformed JSON — swallow silently (binary may print warnings before JSON).
        return [];
    }
    if (!Array.isArray(items)) {
        return [];
    }
    return items
        .filter((item) => item.path === doc.uri.fsPath)
        .map((item) => {
        // Convert 1-based Basilisk coordinates to 0-based VSCode positions.
        const start = new vscode.Position(item.line - 1, item.col - 1);
        const end = new vscode.Position(item.end_line - 1, item.end_col - 1);
        const range = new vscode.Range(start, end);
        const severity = item.severity === "error"
            ? vscode.DiagnosticSeverity.Error
            : vscode.DiagnosticSeverity.Warning;
        const diag = new vscode.Diagnostic(range, `${item.message} [${item.code}]`, severity);
        diag.source = "basilisk";
        diag.code = {
            value: item.code,
            target: vscode.Uri.parse(`https://basilisk-lang.org/errors/${item.code}`),
        };
        return diag;
    });
}
function workspaceRoot() {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}
//# sourceMappingURL=extension.js.map