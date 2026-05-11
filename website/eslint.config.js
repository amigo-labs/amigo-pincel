import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';

export default ts.config(
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs.recommended,
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        parser: ts.parser,
      },
    },
  },
  {
    files: ['**/*.svelte.ts', '**/*.svelte.js'],
    languageOptions: {
      parser: ts.parser,
    },
  },
  {
    rules: {
      // The marketing site uses static href="/path" links throughout. We don't
      // need SvelteKit's resolve() helper for prerendered marketing pages, and
      // forcing it everywhere would add noise without value.
      'svelte/no-navigation-without-resolve': 'off',
    },
  },
  {
    ignores: ['build/', '.svelte-kit/', 'package/', 'node_modules/'],
  },
);
