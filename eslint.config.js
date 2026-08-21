import babelParser from '@babel/eslint-parser'
import eslint from '@eslint/js'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import globals from 'globals'

const createForbiddenNodeRule = (selector, message) => ({
  meta: {
    type: 'problem',
    schema: [],
    messages: { forbidden: message },
  },
  create: (context) => ({
    [selector]: (node) => context.report({ node, messageId: 'forbidden' }),
  }),
})

const localTypeSafetyPlugin = {
  rules: {
    'no-explicit-any': createForbiddenNodeRule(
      'TSAnyKeyword',
      'Explicit any types are forbidden.',
    ),
    'no-non-null-assertion': createForbiddenNodeRule(
      'TSNonNullExpression',
      'Non-null assertions are forbidden.',
    ),
    'no-enum': createForbiddenNodeRule(
      'TSEnumDeclaration',
      'Enums are forbidden. Use literal constants and union types.',
    ),
    'no-type-assertion': {
      meta: {
        type: 'problem',
        schema: [],
        messages: { forbidden: 'Type assertions are forbidden.' },
      },
      create: (context) => ({
        TSAsExpression: (node) => {
          const annotation = node.typeAnnotation
          const isConstAssertion =
            annotation.type === 'TSTypeReference' &&
            annotation.typeName.type === 'Identifier' &&
            annotation.typeName.name === 'const'

          if (!isConstAssertion) {
            context.report({ node, messageId: 'forbidden' })
          }
        },
        TSTypeAssertion: (node) =>
          context.report({ node, messageId: 'forbidden' }),
      }),
    },
    'no-ts-diagnostic-suppression': {
      meta: {
        type: 'problem',
        schema: [],
        messages: { forbidden: 'TypeScript diagnostic suppressions are forbidden.' },
      },
      create: (context) => ({
        Program: () => {
          for (const comment of context.sourceCode.getAllComments()) {
            if (/@ts-(?:ignore|expect-error)\b/u.test(comment.value)) {
              context.report({ node: comment, messageId: 'forbidden' })
            }
          }
        },
      }),
    },
  },
}

export default [
  { ignores: ['.omo', 'dist', 'node_modules', 'worker-configuration.d.ts'] },
  eslint.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 'latest',
      parser: babelParser,
      parserOptions: {
        requireConfigFile: false,
        babelOptions: {
          parserOpts: {
            plugins: ['typescript', 'jsx'],
          },
        },
      },
      globals: {
        ...globals.browser,
        ...globals.worker,
      },
    },
    plugins: {
      'local-type-safety': localTypeSafetyPlugin,
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      ...reactHooks.configs.flat.recommended.rules,
      'local-type-safety/no-explicit-any': 'error',
      'local-type-safety/no-non-null-assertion': 'error',
      'local-type-safety/no-enum': 'error',
      'local-type-safety/no-type-assertion': 'error',
      'local-type-safety/no-ts-diagnostic-suppression': 'error',
      'no-undef': 'off',
      'no-unused-vars': 'off',
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
    },
  },
]
