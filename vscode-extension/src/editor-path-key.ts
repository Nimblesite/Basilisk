// Implements [VSIX-CI-PLATFORM-COVERAGE] path keying. See docs/specs/VSIX-SPEC.md#VSIX-CI-PLATFORM-COVERAGE
/**
 * Keying a file path so a runtime's paths and the editor's agree.
 *
 * Every in-editor overlay Basilisk paints — the CPU heat map, the memory track,
 * the leak badges — matches rows produced by a Python runtime against the
 * editors the user has open. Those two paths come from different producers and
 * are only textually identical on POSIX: the runtime reports the interpreter's
 * own filename, while `Uri.fsPath` hands back what VS Code resolved.
 *
 * On Windows they disagree in two ways — the drive letter's case, and 8.3 short
 * components — so a raw string compare NEVER matches there and the overlay
 * silently paints nothing: the data is correct, the editor just stays blank.
 * That is a whole-feature outage that looks like "no results", which is exactly
 * why it survived until win32 ran in CI.
 *
 * Shared rather than copied: the profiler learned this first, and the memory
 * decorations had the identical latent bug.
 */

import * as path from "path";
import * as fs from "fs";

/**
 * Key a path for cross-producer comparison.
 *
 * Windows paths are case-insensitive, so folding case there is sound; POSIX
 * paths are case-SENSITIVE, so it must not fold there.
 */
export function editorPathKey(file: string): string {
  return process.platform === "win32" ? expandedWindowsPath(file).toLowerCase() : path.resolve(file);
}

/**
 * A Windows path with any 8.3 short component expanded to its real name.
 *
 * Case-folding alone is not enough. The two producers reach the same file by
 * different routes: `os.tmpdir()` yields the short form Windows keeps for
 * legacy callers (`C:\Users\RUNNER~1\…`), while a path the debug adapter or the
 * editor resolved carries the long one (`C:\Users\runneradmin\…`). Those differ
 * in more than case, so `path.resolve` leaves them unequal and the overlay
 * matches nothing.
 *
 * `realpathSync.native` is what collapses the two spellings — it asks the
 * filesystem for the name it actually records. It therefore touches disk and
 * throws for a path that no longer exists, so a file that has since been
 * deleted falls back to the resolved-but-unexpanded form rather than taking the
 * whole decoration pass down with it. Callers memoise (see `pathKeyer`) so this
 * costs one lookup per distinct file, not one per hot line.
 */
function expandedWindowsPath(file: string): string {
  try {
    return fs.realpathSync.native(file);
  } catch {
    return path.resolve(file);
  }
}

/**
 * `editorPathKey` memoised for the length of one decoration pass.
 *
 * A profile or snapshot carries many rows across few files, and the expansion
 * above hits the filesystem — so the same handful of paths would otherwise be
 * looked up hundreds of times. The cache is per-pass rather than module-level
 * so a file that moves between runs is never matched against a stale name.
 */
export function pathKeyer(): (file: string) => string {
  const cache = new Map<string, string>();
  return (file: string): string => {
    const hit = cache.get(file);
    if (hit !== undefined) { return hit; }
    const key = editorPathKey(file);
    cache.set(file, key);
    return key;
  };
}
