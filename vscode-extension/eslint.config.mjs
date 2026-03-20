import tseslint from 'typescript-eslint';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);
const { masterRules, testOverrides } = require('./eslint-rules.cjs');

export default tseslint.config(
  // Global ignores
  {
    ignores: ['out/', 'coverage/', 'scripts/', '*.config.*', '.vscode-test/', 'node_modules/', '.vscode-test.mjs', 'eslint-rules.cjs'],
  },

  // Base configs: strict + stylistic type-checked
  ...tseslint.configs.strictTypeChecked,
  ...tseslint.configs.stylisticTypeChecked,

  // Main source rules
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      ...masterRules,
      // Project-specific: VSIX uses interfaces for VSCode API compat
      '@typescript-eslint/consistent-type-definitions': ['error', 'interface'],
      // Project-specific: VSIX uses function declarations for hoisting
      'func-style': ['error', 'declaration'],
      // Matches CLAUDE.md 500 LOC rule. extension.ts (1120 lines) needs splitting.
      'max-lines': ['error', { max: 500, skipBlankLines: true, skipComments: true }],
      'max-params': ['error', 3],
      // Project-specific: class methods that implement TreeDataProvider
      'class-methods-use-this': ['error', {
        exceptMethods: ['getTreeItem', 'createDebugAdapterTracker'],
      }],
      // Disabled: VSCode API types are not readonly-compatible and the allow
      // list would be enormous. Enable incrementally when the codebase is smaller.
      '@typescript-eslint/prefer-readonly-parameter-types': 'off',
      // VSCode extension code has many defensive null checks on values that
      // TypeScript thinks are non-null but can be undefined at runtime.
      '@typescript-eslint/no-unnecessary-condition': 'off',
      // Allow broad template expression types — DAP/LSP messages log unknown values.
      '@typescript-eslint/restrict-template-expressions': 'off',
    },
  },

  // Test file overrides
  {
    files: ['**/*.test.ts', '**/*.spec.ts', '**/test-helpers.ts', '**/test/**/*.ts'],
    rules: testOverrides,
  },
);
