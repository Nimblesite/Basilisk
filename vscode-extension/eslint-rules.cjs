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

  // ── Complexity limits ────────────────────────────────────────────────
  'max-depth': ['error', 4],
  'max-lines-per-function': ['error', { max: 60, skipBlankLines: true, skipComments: true }],
  'complexity': ['error', 15],
};

/** @type {Record<string, import('eslint').Linter.RuleEntry>} */
const testOverrides = {
  // Tests often need longer functions and more nesting.
  'max-lines-per-function': 'off',
  'max-lines': 'off',
  'max-depth': ['error', 5],
  'complexity': 'off',
  '@typescript-eslint/no-non-null-assertion': 'off',
  // Tests may use any for mocking.
  '@typescript-eslint/no-explicit-any': 'off',
  '@typescript-eslint/no-unsafe-assignment': 'off',
  '@typescript-eslint/no-unsafe-member-access': 'off',
  '@typescript-eslint/no-unsafe-call': 'off',
  '@typescript-eslint/no-unsafe-argument': 'off',
  '@typescript-eslint/no-unsafe-return': 'off',
  // Tests use string truthiness checks extensively.
  '@typescript-eslint/strict-boolean-expressions': 'off',
  // Test helpers may use require() for dynamic loading.
  '@typescript-eslint/no-require-imports': 'off',
  // Tests don't need explicit member accessibility.
  '@typescript-eslint/explicit-member-accessibility': 'off',
  // Test helpers return bare promises for chaining.
  '@typescript-eslint/promise-function-async': 'off',
};

module.exports = { masterRules, testOverrides };
