import js from "@eslint/js";
import tseslint from "typescript-eslint";

// Orchester's rules for all of web/ (AGENTS.md; server.md row 64): files under 600 lines, functions of at most 80
// lines and complexity 25, no user-facing text or style values in components. @orchester/ui has no words at all: its
// components take them from the app as props.
// The props of @orchester/ui components that are shown as text (`ui/components/*.tsx`).
const KIT_TEXT_PROPS = [
  "label", "busyLabel", "title", "description", "meta", "text", "hint", "question", "link", "promise", "product",
  "revealLabel", "concealLabel", "body",
].join("|");

const noLiterals = [
  {
    selector: "JSXText[value=/[A-Za-z0-9]/]",
    message: "Text a visitor reads comes from the app's locales (t(\"…\")), or as a prop in @orchester/ui.",
  },
  // HTML attributes a reader hears or sees, and every prop the UI kit renders as text.
  ...[
    "JSXAttribute[name.name=/^(aria-label|placeholder|title|alt|aria-description)$/] > Literal",
    `JSXAttribute[name.name=/^(${KIT_TEXT_PROPS})$/] > Literal`,
    `JSXAttribute[name.name=/^(${KIT_TEXT_PROPS})$/] > JSXExpressionContainer > Literal`,
    `JSXAttribute[name.name=/^(${KIT_TEXT_PROPS})$/] > JSXExpressionContainer > TemplateLiteral`,
  ].map((selector) => ({
    selector,
    message: "Text a visitor reads comes from the app's locales (t(\"…\")), or as a prop in @orchester/ui.",
  })),
  {
    selector: "JSXAttribute[name.name='style']",
    message: "Styles come from CSS modules that use the theme tokens; no inline styles (the CSP refuses them too).",
  },
  {
    selector: "Literal[value=/^#[0-9a-fA-F]{3,8}$/]",
    message: "Colours live only in the theme files.",
  },
];

export default tseslint.config(
  { ignores: ["**/.next/**", "**/node_modules/**", "**/next-env.d.ts", "**/test-results/**", "**/playwright-report/**"] },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    rules: {
      "max-lines": ["error", { max: 600 }],
      "max-lines-per-function": ["error", { max: 80, skipBlankLines: false, skipComments: false }],
      complexity: ["error", 25],
      "no-restricted-syntax": ["error", ...noLiterals],
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: true }],
    },
  },
  {
    files: ["**/*.mjs", "**/*.config.ts"],
    ...tseslint.configs.disableTypeChecked,
  },
);
