// Implements [WITHDRAWAL-SURFACES] and [WITHDRAWAL-INERT].
// See docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-SURFACES
/**
 * Basilisk for VS Code — a notice.
 *
 * Basilisk's type checker was producing incorrect results, so this extension no
 * longer contains one. It bundles no binary, starts no language server,
 * publishes no diagnostic, and contributes no setting: there is nothing left
 * for a user to configure or an editor to run. It exists only so that an
 * already-installed copy tells its owner what happened and how to remove it.
 *
 * The statement is NOT authored here. `withdrawal-notice.ts` is generated from
 * the messaging spec by scripts/gen_withdrawal_copy.py and drift-gated in CI
 * ([WITHDRAWAL-INERT-TEXT]), so this extension cannot say its own version of it.
 */

import * as vscode from "vscode";
import { WITHDRAWAL_NOTICE } from "./withdrawal-notice";

/** The full statement lives here; the notice points at it. */
export const STATEMENT_URL = "https://www.basilisk-python.dev/";

/** Virtual-document scheme for the read-only statement. */
export const NOTICE_SCHEME = "basilisk-notice";

/** The statement, opened by `basilisk.showStatement`. */
export const NOTICE_URI = vscode.Uri.parse(`${NOTICE_SCHEME}:Basilisk is unlisted.md`);

/** Command that opens the statement in the editor. */
export const SHOW_STATEMENT_COMMAND = "basilisk.showStatement";

/** `globalState` key holding the version whose notice has been shown. */
export const ANNOUNCED_KEY = "basilisk.announcedVersion";

/** Label of the notification action that opens the statement. */
export const READ_ACTION = "Read the statement";

/** One line, because the rest of the message is a click away. */
export const ANNOUNCEMENT =
    "Basilisk is unlisted. Its type checker was producing incorrect results and is no longer part of this extension — it checks nothing. Uninstall Basilisk.";

/** Asks the user something and resolves with the action they chose. */
export type Prompt = (message: string, action: string) => Thenable<string | undefined>;

/**
 * The slice of `vscode.Memento` the announcement needs. Narrowing the
 * dependency to two methods is what lets the once-per-version rule be tested
 * without a fabricated `ExtensionContext`.
 */
export interface AnnouncementState {
    get(key: string): string | undefined;
    update(key: string, value: string): Thenable<void>;
}

/**
 * The document text: the approved notice, plus a pointer to the full statement.
 * The pointer is a link, not a restatement — no surface writes its own version
 * of the message.
 */
export function statementText(): string {
    return `${WITHDRAWAL_NOTICE}\nThe full statement: ${STATEMENT_URL}\n`;
}

/**
 * Whether this activation should interrupt the user.
 *
 * Once per installed version. Silence would leave the checker's owner none the
 * wiser, and re-announcing on every window would be nagging about something
 * they cannot fix from here.
 */
export function shouldAnnounce(announced: string | undefined, version: string): boolean {
    return announced !== version;
}

/** Narrows an unknown value to an indexable object. */
function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null;
}

/** The installed version, or `"unknown"` when the manifest cannot be read. */
export function extensionVersion(packageJson: unknown): string {
    if (isRecord(packageJson) && typeof packageJson.version === "string") {
        return packageJson.version;
    }
    return "unknown";
}

/** Open the statement beside whatever the user was doing. */
export async function showStatement(): Promise<void> {
    const document = await vscode.workspace.openTextDocument(NOTICE_URI);
    await vscode.window.showTextDocument(document, { preview: false });
}

/** The real prompt: a warning, not a tip — their build changed. */
function warn(message: string, action: string): Thenable<string | undefined> {
    return vscode.window.showWarningMessage(message, action);
}

/** Tell the user once per version, then stop. */
export async function announce(
    state: AnnouncementState,
    version: string,
    prompt: Prompt = warn,
): Promise<void> {
    if (!shouldAnnounce(state.get(ANNOUNCED_KEY), version)) {
        return;
    }
    await state.update(ANNOUNCED_KEY, version);
    if ((await prompt(ANNOUNCEMENT, READ_ACTION)) === READ_ACTION) {
        await showStatement();
    }
}

export function activate(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
        vscode.workspace.registerTextDocumentContentProvider(NOTICE_SCHEME, {
            provideTextDocumentContent: statementText,
        }),
        vscode.commands.registerCommand(SHOW_STATEMENT_COMMAND, showStatement),
    );
    void announce(context.globalState, extensionVersion(context.extension.packageJSON));
}

export function deactivate(): void {
    // Nothing is started, so nothing needs stopping.
}
