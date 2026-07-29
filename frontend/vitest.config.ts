import { defineConfig } from 'vitest/config'

// Separate from `vite.config.ts` on purpose: the suite covers the framework-free
// domain modules — reducers, parsers, selectors, codecs — so it needs neither the
// React plugin nor the Tailwind plugin, and no DOM. Anything that requires a
// renderer belongs in a config that supplies one, not in this one by default.
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
    // Explicit imports from 'vitest' instead of injected globals, so the test files
    // typecheck under the same `tsconfig.app.json` as the code they cover.
    globals: false,
  },
})
