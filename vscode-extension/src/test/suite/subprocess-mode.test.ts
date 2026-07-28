// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * Contract tests for the subprocess-mode parse boundary.
 *
 * Subprocess mode publishes whatever `basilisk check --output json` reports, so
 * this boundary decides what the editor shows when the LSP is off. It had no
 * tests, and two blind spots lived in the gap: an entry the CLI emits for a
 * file it could not parse was dropped for lacking a rule code, and a run that
 * exited 3 published nothing at all. Both are pinned here.
 */

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { parseDiagnostics } from "../../subprocess-mode";

/** The docs host every coded diagnostic links to. */
const DOCS_HOST = "https://www.basilisk-python.dev/errors/";

/** A document on disk, so `uri.fsPath` matches what the CLI reports. */
async function scratchDocument(): Promise<vscode.TextDocument> {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "basilisk-subproc-"));
  const file = path.join(dir, "src.py");
  fs.writeFileSync(file, "x: int = 1\n");
  return vscode.workspace.openTextDocument(vscode.Uri.file(file));
}

/** One CLI JSON entry for `doc`, with `overrides` applied. */
function entry(doc: vscode.TextDocument, overrides: Record<string, unknown>): string {
  return JSON.stringify([
    {
      code: "BSK-0001",
      severity: "error",
      message: "Missing parameter type annotation for `x`",
      path: doc.uri.fsPath,
      line: 2,
      col: 3,
      end_line: 2,
      end_col: 7,
      ...overrides,
    },
  ]);
}

suite("Subprocess mode report parsing [VSIX]", () => {
  let doc: vscode.TextDocument;

  suiteSetup(async () => {
    doc = await scratchDocument();
  });

  test("a coded diagnostic keeps its code, label and docs link", () => {
    const [diag] = parseDiagnostics(entry(doc, {}), doc);
    assert.ok(diag, "a matching entry must produce a diagnostic");
    assert.strictEqual(
      diag.message,
      "Missing parameter type annotation for `x` [BSK-0001]",
      "the label carries the code in brackets",
    );
    assert.strictEqual(diag.severity, vscode.DiagnosticSeverity.Error, "error maps to Error");
    assert.strictEqual(diag.source, "basilisk", "the diagnostic is attributed to Basilisk");
    assert.strictEqual(diag.range.start.line, 1, "line is converted to 0-based");
    assert.strictEqual(diag.range.start.character, 2, "column is converted to 0-based");
    assert.strictEqual(diag.range.end.line, 1, "the end line is converted too");
    assert.strictEqual(diag.range.end.character, 6, "the end column is converted too");
    assert.ok(typeof diag.code === "object", "a coded diagnostic links to its docs page");
    assert.strictEqual(diag.code.value, "BSK-0001", "the code is carried verbatim");
    assert.strictEqual(
      diag.code.target.toString(),
      `${DOCS_HOST}BSK-0001`,
      "the target is that code's page",
    );
  });

  // The CLI reports a file it could not parse with a null code, because no rule
  // produced the entry. Requiring a code dropped it, so the editor stayed clean
  // for a file that never got checked.
  test("a file the CLI could not parse is published, not dropped", () => {
    const report = entry(doc, {
      code: null,
      message: `syntax error in ${doc.uri.fsPath}: Expected \`:\`, found newline`,
      line: 1,
      col: 1,
      end_line: 1,
      end_col: 1,
    });
    const diagnostics = parseDiagnostics(report, doc);
    assert.strictEqual(diagnostics.length, 1, "the parse failure must reach the editor");
    const [diag] = diagnostics;
    assert.ok(diag, "the parse failure must produce a diagnostic");
    assert.ok(diag.message.includes("syntax error"), "the message says why the file failed");
    assert.ok(!diag.message.includes("[null]"), "a missing code must not render as [null]");
    assert.ok(!diag.message.includes("undefined"), "a missing code must not render as undefined");
    assert.strictEqual(diag.severity, vscode.DiagnosticSeverity.Error, "a failure is an error");
    assert.strictEqual(diag.source, "basilisk", "the failure is attributed to Basilisk");
    assert.strictEqual(diag.code, undefined, "no rule ran, so no code is claimed");
    assert.strictEqual(diag.range.start.line, 0, "the failure anchors at the first line");
    assert.strictEqual(diag.range.start.character, 0, "the failure anchors at the first column");
  });

  test("severity that is not error is reported as a warning", () => {
    const [diag] = parseDiagnostics(entry(doc, { severity: "warning" }), doc);
    assert.ok(diag, "a warning entry must produce a diagnostic");
    assert.strictEqual(diag.severity, vscode.DiagnosticSeverity.Warning, "warning maps to Warning");
  });

  test("entries for other files never leak into this document", () => {
    const report = entry(doc, { path: path.join(os.tmpdir(), "someone-elses.py") });
    assert.deepStrictEqual(parseDiagnostics(report, doc), [], "another file's entry is filtered");
  });

  test("a malformed payload degrades to nothing, never to a throw", () => {
    assert.deepStrictEqual(parseDiagnostics("", doc), [], "empty output yields no diagnostics");
    assert.deepStrictEqual(parseDiagnostics("not json", doc), [], "garbage yields no diagnostics");
    assert.deepStrictEqual(parseDiagnostics("[]", doc), [], "a clean run yields no diagnostics");
    assert.deepStrictEqual(parseDiagnostics("{}", doc), [], "a non-array payload is rejected");
    assert.deepStrictEqual(parseDiagnostics("null", doc), [], "a null payload is rejected");
    assert.deepStrictEqual(parseDiagnostics("[1, 2, 3]", doc), [], "scalar entries are rejected");
  });

  test("an entry missing a required field is dropped rather than guessed", () => {
    for (const field of ["message", "path", "line", "col", "end_line", "end_col"]) {
      const report = entry(doc, { [field]: undefined });
      assert.deepStrictEqual(
        parseDiagnostics(report, doc),
        [],
        `an entry without "${field}" must be dropped`,
      );
    }
  });

  test("a field of the wrong type is dropped rather than coerced", () => {
    assert.deepStrictEqual(parseDiagnostics(entry(doc, { line: "2" }), doc), [], "line must be a number");
    assert.deepStrictEqual(parseDiagnostics(entry(doc, { message: 7 }), doc), [], "message must be a string");
    assert.deepStrictEqual(parseDiagnostics(entry(doc, { path: 7 }), doc), [], "path must be a string");
  });

  test("several entries are published in the order the CLI reported them", () => {
    const report = JSON.stringify([
      { code: "BSK-0001", severity: "error", message: "first", path: doc.uri.fsPath, line: 1, col: 1, end_line: 1, end_col: 2 },
      { code: null, severity: "error", message: "syntax error in src.py", path: doc.uri.fsPath, line: 1, col: 1, end_line: 1, end_col: 1 },
      { code: "BSK-0002", severity: "warning", message: "third", path: doc.uri.fsPath, line: 3, col: 1, end_line: 3, end_col: 2 },
    ]);
    const diagnostics = parseDiagnostics(report, doc);
    assert.strictEqual(diagnostics.length, 3, "every entry for this file is published");
    assert.ok(diagnostics[0]?.message.startsWith("first"), "the first entry stays first");
    assert.ok(diagnostics[1]?.message.includes("syntax error"), "the failure keeps its place");
    assert.ok(diagnostics[2]?.message.startsWith("third"), "the last entry stays last");
    assert.strictEqual(diagnostics[1]?.code, undefined, "only the failure lacks a code");
    assert.ok(diagnostics[0]?.code !== undefined, "coded entries keep their code");
    assert.ok(diagnostics[2]?.code !== undefined, "coded entries keep their code");
  });
});
