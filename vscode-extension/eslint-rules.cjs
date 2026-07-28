// Shared ESLint rules for all Basilisk JS/TS projects.
//
// Import in each project's eslint.config.mjs:
//   const { masterRules, testOverrides } = require('./eslint-rules.cjs');

/** @type {Record<string, import('eslint').Linter.RuleEntry>} */
const masterRules = {
  // ── Style & Readability ──────────────────────────────────────────────
  'no-console': 'error',
  'no-debugger': 'error',
  'no-eval': 'error',
  'no-implied-eval': 'error',
  'no-var': 'error',
  'prefer-const': 'error',
  'prefer-template': 'error',
  'eqeqeq': ['error', 'always'],
  'curly': ['error', 'all'],
  'no-throw-literal': 'error',

  // ── TypeScript — type safety (CRITICAL) ─────────────────────────────
  '@typescript-eslint/no-unused-vars': ['error', {
    argsIgnorePattern: '^_',
    varsIgnorePattern: '^_',
  }],
  '@typescript-eslint/explicit-function-return-type': ['error', {
    allowExpressions: true,
    allowTypedFunctionExpressions: true,
  }],
  '@typescript-eslint/no-explicit-any': 'error',
  '@typescript-eslint/no-non-null-assertion': 'error',
  '@typescript-eslint/restrict-template-expressions': ['error', {
    allowNumber: true,
    allowBoolean: true,
  }],

  // ── TypeScript — strict safety (NEW) ───────────────────────────────
  // 1. Ban unsafe any operations — catch type holes at boundaries
  '@typescript-eslint/no-unsafe-assignment': 'error',
  '@typescript-eslint/no-unsafe-call': 'error',
  '@typescript-eslint/no-unsafe-member-access': 'error',
  '@typescript-eslint/no-unsafe-return': 'error',
  '@typescript-eslint/no-unsafe-argument': 'error',
  // 2. Require await on promises — prevent silent swallowed errors
  '@typescript-eslint/no-floating-promises': 'error',
  '@typescript-eslint/no-misused-promises': 'error',
  // 3. Catch unknown — force safe error handling
  '@typescript-eslint/use-unknown-in-catch-callback-variable': 'error',
  // 4. Require async functions to actually await — no misleading signatures
  '@typescript-eslint/require-await': 'error',
  // 5. Ban require() — ESM imports only, full type coverage
  '@typescript-eslint/no-require-imports': 'error',
  // 6. Prefer nullish coalescing — null-safe defaults
  '@typescript-eslint/prefer-nullish-coalescing': 'error',
  // 7. No extraneous classes — prefer modules over empty class wrappers
  '@typescript-eslint/no-extraneous-class': 'error',
  // 8. Consistent type imports — tree-shaking and clarity
  '@typescript-eslint/consistent-type-imports': ['error', {
    prefer: 'type-imports',
    fixStyle: 'inline-type-imports',
  }],
  // 9. No redundant type constituents — catch `string | string` noise
  '@typescript-eslint/no-redundant-type-constituents': 'error',
  // 10. Explicit member accessibility — public/private must be declared
  '@typescript-eslint/explicit-member-accessibility': ['error', {
    accessibility: 'explicit',
    overrides: { constructors: 'no-public' },
  }],
  // 11. Strict boolean expressions — no truthy coercion bugs
  '@typescript-eslint/strict-boolean-expressions': ['error', {
    allowString: false,
    allowNumber: false,
    allowNullableObject: true,
    allowNullableBoolean: true,
    allowNullableString: false,
    allowNullableNumber: false,
    allowAny: false,
  }],
  // 12. Switch exhaustiveness — every enum case must be handled
  '@typescript-eslint/switch-exhaustiveness-check': 'error',
  // 13. No confusing void expression — void only in statements, not values
  '@typescript-eslint/no-confusing-void-expression': ['error', {
    ignoreArrowShorthand: true,
    ignoreVoidOperator: false,
  }],
  // 14. No unnecessary type assertion — remove useless `as X` casts
  '@typescript-eslint/no-unnecessary-type-assertion': 'error',
  // 15. Await thenable only — catch `await nonPromise` bugs
  '@typescript-eslint/await-thenable': 'error',
  // 16. No unsafe enum comparison — prevent enum vs unrelated value comparisons
  '@typescript-eslint/no-unsafe-enum-comparison': 'error',
  // 17. Promise function async — if it returns a Promise, mark it async
  '@typescript-eslint/promise-function-async': 'error',

  // ── No literals — use NAMED CONSTANTS ───────────────────────────────
  // 18. No magic numbers — every number must be a named constant
  '@typescript-eslint/no-magic-numbers': ['error', {
    ignore: [-1, 0, 1, 2],
    ignoreEnums: true,
    ignoreNumericLiteralTypes: true,
    ignoreReadonlyClassProperties: true,
    ignoreTypeIndexes: true,
    enforceConst: true,
    detectObjects: true,
  }],
  // 19. Prefer enum member values — no bare string/number enum values
  '@typescript-eslint/prefer-enum-initializers': 'error',
  // 20. No duplicate string literals — extract to named constants
  '@typescript-eslint/no-duplicate-type-constituents': 'error',

  // ── Defensive correctness ─────────────────────────────────────────────
  // 21. Return-await in try/catch — catch async errors properly
  '@typescript-eslint/return-await': ['error', 'in-try-catch'],
  // 22. No unnecessary boolean literal compare — `if (x === true)` is noise
  '@typescript-eslint/no-unnecessary-boolean-literal-compare': 'error',

  // ── Unchecked escape hatches & async hazards (CRITICAL) ──────────────
  // 23. No unsafe type assertion — `as T` is the last hole left once
  //     no-explicit-any and the no-unsafe-* family are on. Narrow untrusted
  //     payloads (DAP/LSP JSON, DOM lookups) with a runtime check instead.
  '@typescript-eslint/no-unsafe-type-assertion': 'error',
  // 24. No shadowing — an inner binding that reuses an outer name silently
  //     detaches later reads from the value the author meant.
  '@typescript-eslint/no-shadow': 'error',
  // 25. Async race conditions — `x = await f()` after another await can clobber
  //     an interleaved write to the same variable.
  'require-atomic-updates': 'error',
  // 26. Nothing may be returned from a `new Promise` executor — the value is
  //     discarded, so a `return` there is always a mistake.
  'no-promise-executor-return': 'error',
  // 27. `'${x}'` in a plain string never interpolates — it ships the literal
  //     dollar-brace text to the user.
  'no-template-curly-in-string': 'error',

  // ── Complexity limits ────────────────────────────────────────────────
  'max-depth': ['error', 4],
  'max-lines-per-function': ['error', { max: 60, skipBlankLines: true, skipComments: true }],
  'complexity': ['error', 15],
};

/** @type {Record<string, import('eslint').Linter.RuleEntry>} */
const testOverrides = {
  // Tests use literal line/column numbers for assertions — unavoidable
  '@typescript-eslint/no-magic-numbers': 'off',
  // Test helpers may be defined for future use or conditionally used
  '@typescript-eslint/no-unused-vars': ['error', {
    argsIgnorePattern: '^_',
    varsIgnorePattern: '^_|^pollUntilResult|^activate',
  }],
  // Test files can be longer — each test suite is a cohesive unit
  'max-lines': ['error', { max: 1000, skipBlankLines: true, skipComments: true }],
  // Test functions often need async for the framework even without await
  '@typescript-eslint/require-await': 'off',
  // Tests may await setup/teardown helpers that return void
  '@typescript-eslint/await-thenable': 'off',
  '@typescript-eslint/no-confusing-void-expression': 'off',
  // Tests use truthy checks on dynamic/unknown values from LSP responses
  '@typescript-eslint/strict-boolean-expressions': 'off',
  // NOTE: `@typescript-eslint/no-unsafe-type-assertion` is deliberately NOT
  // overridden here. Turning it off for tests was measured to hide 101 of 132
  // violations, of which only a handful are the generic-test-double case the
  // exemption was written for — a test that asserts a payload's shape proves
  // nothing about the payload. The few sites where overriding a generic API
  // (`sendRequest<R>(): Promise<R>`) genuinely cannot be satisfied carry a
  // targeted `eslint-disable-next-line` naming that reason, so the exemption is
  // visible at the site instead of blanketing the suite.
  // Test setup/teardown functions can be longer
  'max-lines-per-function': ['error', { max: 120, skipBlankLines: true, skipComments: true }],
};

module.exports = { masterRules, testOverrides };
