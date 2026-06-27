// Implements [LSPARCH-FEATURES-HOVER] / [LSPARCH-FEATURES-DEFINITION].
// See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
/**
 * Shared Python fixtures for the hover (lsp-hover) and goto (lsp-goto)
 * hammer suites. Kept in one place so both suites exercise the SAME rich
 * symbol set and expected definition lines are derived (via `locate`) rather
 * than hard-coded — the fixture cannot drift out from under the assertions.
 *
 * Tokens are deliberately distinct words so `locate(SUBJECT_SOURCE, token, n)`
 * resolves unambiguously to a definition vs reference site.
 */

/** Cross-file helper module imported by the subject file. */
export const HELPER_SOURCE = [
    '"""Helper module for cross-file navigation."""',
    '',
    'def helper_fn() -> None:',
    '    """A helper function."""',
    '    return None',
    '',
    'class HelperClass:',
    '    """A helper class."""',
    '    member: int = 0',
    '',
].join('\n');

/** Filename the subject file imports from; written into the same tmpDir. */
export const HELPER_FILENAME = 'nav_helper.py';

/** One rich subject file packed with every hover/goto-relevant symbol kind. */
export const SUBJECT_SOURCE = [
    '"""Module docstring for navigation subjects."""',    // 0
    'from typing import Final',                            // 1
    'from nav_helper import helper_fn, HelperClass',       // 2
    '',                                                    // 3
    'PI: Final = 3.14',                                    // 4
    'counter = 5',                                         // 5
    '',                                                    // 6
    'def calculate(operand: int) -> int:',                // 7
    '    """Compute the square of operand."""',           // 8
    '    squared = operand * operand',                     // 9
    '    return squared',                                  // 10
    '',                                                    // 11
    'class Widget:',                                       // 12
    '    """A configurable widget."""',                   // 13
    '    width: int = 10',                                 // 14
    '',                                                    // 15
    '    def resize(self, factor: int) -> int:',          // 16
    '        """Resize the widget by a factor."""',       // 17
    '        return self.width * factor',                  // 18
    '',                                                    // 19
    'result: int = calculate(5)',                          // 20
    'gadget: Widget = Widget()',                           // 21
    'helper_fn()',                                         // 22
    'instance: HelperClass = HelperClass()',              // 23
    '',                                                    // 24
].join('\n');
