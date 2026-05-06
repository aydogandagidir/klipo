/* eslint-env node */
module.exports = {
  root: true,
  env: { browser: true, es2022: true, node: true },
  extends: [
    "eslint:recommended",
    "plugin:@typescript-eslint/recommended",
    "plugin:react-hooks/recommended",
    "plugin:react/recommended",
    "plugin:react/jsx-runtime",
    "prettier",
  ],
  parser: "@typescript-eslint/parser",
  parserOptions: {
    ecmaVersion: 2022,
    sourceType: "module",
    ecmaFeatures: { jsx: true },
  },
  settings: {
    react: { version: "18.3" },
  },
  plugins: ["@typescript-eslint", "react-refresh"],
  ignorePatterns: [
    "dist",
    "node_modules",
    "src-tauri/target",
    "bench/target",
    "**/*.config.*",
    "**/*.cjs",
  ],
  rules: {
    // The Klipo non-negotiable: no `any`. Use `unknown` and narrow.
    "@typescript-eslint/no-explicit-any": "error",
    "@typescript-eslint/no-unused-vars": [
      "error",
      { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
    ],
    "react-refresh/only-export-components": [
      "warn",
      { allowConstantExport: true },
    ],
    "no-console": ["warn", { allow: ["warn", "error"] }],
  },
};
