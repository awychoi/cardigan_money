import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "node_modules", ".cache", ".toolchain", "playwright-report", "test-results"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
      "no-restricted-properties": [
        "error",
        { object: "window", property: "open", message: "New windows are blocked; no renderer-initiated navigation." },
        { object: "document", property: "write", message: "No document.write." },
      ],
      "no-restricted-syntax": [
        "error",
        { selector: "WithStatement", message: "with is forbidden" },
        { selector: "CallExpression[callee.name='eval']", message: "eval is forbidden" },
        { selector: "NewExpression[callee.name='Function']", message: "new Function is forbidden" },
      ],
    },
  },
  {
    files: ["e2e/**/*.ts", "*.config.ts"],
    rules: { "@typescript-eslint/no-non-null-assertion": "off" },
  },
);
