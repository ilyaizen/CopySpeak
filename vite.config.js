import tailwindcss from "@tailwindcss/vite";
import { defineConfig, lazyPlugins } from "vite-plus";
import { sveltekit } from "@sveltejs/kit/vite";

const host = process.env.TAURI_DEV_HOST;

const agentToolingIgnores = [
  ".agent/**",
  ".agents/**",
  ".claude/**",
  ".codex/**",
  ".continue/**",
  ".cursor/**",
  ".gemini/**",
  ".hermes/**",
  ".opencode/**",
  ".pi/**",
  ".roo/**",
  ".windsurf/**",
  "tools/oxlint/anti-slop/**"
];

// https://vite.dev/config/
export default defineConfig({
  lint: {
    ignorePatterns: agentToolingIgnores,
    jsPlugins: [
      { name: "vite-plus", specifier: "vite-plus/oxlint-plugin" },
      { name: "anti-slop", specifier: "./tools/oxlint/anti-slop/index.ts" }
    ],
    rules: {
      "vite-plus/prefer-vite-plus-imports": "error",
      "anti-slop/no-chained-type-assertions": "error",
      "anti-slop/no-conditional-empty-object-spread": "error",
      "anti-slop/no-known-value-widening": "error",
      "anti-slop/no-module-mocking": "error",
      "anti-slop/no-object-parameters": "error",
      "anti-slop/no-reflect-apply": "error",
      "anti-slop/no-reflect-get": "error",
      "anti-slop/no-runtime-typeof": "error",
      "anti-slop/no-shape-in-symbol-names": "error",
      "anti-slop/no-unknown-parameters": "error",
      "anti-slop/no-unknown-returns": "error",
      "anti-slop/no-unknown-type-aliases": "error",
      "anti-slop/no-unsafe-dictionary-type": "error",
      "anti-slop/no-widen-then-assert": "error",
      "anti-slop/require-safety-comment-for-type-assertion": "error"
    },
    options: { typeAware: true, typeCheck: true }
  },
  fmt: {
    useTabs: false,
    tabWidth: 2,
    singleQuote: false,
    trailingComma: "none",
    printWidth: 100,
    semi: true,
    endOfLine: "lf",
    sortPackageJson: false,
    sortTailwindcss: {},
    svelte: {
      indentScriptAndStyle: true
    },
    ignorePatterns: [
      "node_modules/",
      ".svelte-kit/",
      "build/",
      "dist/",
      "dist-clean/",
      "src-tauri/target/",
      "coverage/",
      ".git/",
      ".obsidian/",
      "src-tauri/",
      ".planning/",
      ...agentToolingIgnores
    ]
  },
  plugins: lazyPlugins(() => [tailwindcss(), sveltekit()]),
  define: {
    "import.meta.env.VITE_IS_VERCEL": process.env.VERCEL === "1" || process.env.VERCEL === "true"
  },
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,

  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 5174 } : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"]
    }
  }
});
