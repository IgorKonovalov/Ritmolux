import js from '@eslint/js'
import tsPlugin from '@typescript-eslint/eslint-plugin'
import tsParser from '@typescript-eslint/parser'
import react from 'eslint-plugin-react'
import reactHooks from 'eslint-plugin-react-hooks'
import prettier from 'eslint-config-prettier'

/**
 * The process boundary is a lint rule, not a convention (ADR-0178).
 *
 * A renderer file that imports `node:fs`, `electron` or `child_process` is in
 * the wrong process, and the answer is a narrow `window.api` capability rather
 * than the import. The one exception is a `type`-only import of a main-side
 * shape, which erases at compile time and reaches no runtime module.
 */
const NODE_ONLY = [
  'electron',
  'fs',
  'path',
  'child_process',
  'dgram',
  'net',
  'os',
  'node:*',
]

export default [
  { ignores: ['dist/**', 'release/**', 'node_modules/**', '*.config.ts', 'scripts/**'] },
  js.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: { ecmaVersion: 2022, sourceType: 'module', ecmaFeatures: { jsx: true } },
    },
    plugins: { '@typescript-eslint': tsPlugin },
    rules: {
      ...tsPlugin.configs.recommended.rules,
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
      '@typescript-eslint/consistent-type-imports': ['error', { prefer: 'type-imports' }],
      'no-undef': 'off',
    },
  },
  {
    files: ['renderer/**/*.{ts,tsx}'],
    plugins: { react, 'react-hooks': reactHooks },
    settings: { react: { version: '18.3' } },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react/jsx-uses-react': 'off',
      'react/react-in-jsx-scope': 'off',
      'no-restricted-imports': [
        'error',
        {
          paths: NODE_ONLY.filter((name) => !name.endsWith('*')).map((name) => ({
            name,
            message: 'The renderer never imports Node. Add a window.api capability instead.',
          })),
          patterns: [
            {
              group: ['node:*'],
              message: 'The renderer never imports Node. Add a window.api capability instead.',
            },
          ],
        },
      ],
    },
  },
  {
    files: ['**/*.test.ts', '**/*.test.tsx'],
    languageOptions: { globals: { global: 'readonly' } },
    rules: { '@typescript-eslint/no-explicit-any': 'off' },
  },
  prettier,
]
