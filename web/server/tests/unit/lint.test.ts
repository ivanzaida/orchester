import { fileURLToPath } from "node:url";

import { ESLint } from "eslint";
import { describe, expect, it } from "vitest";

/**
 * The literal rule of `web/eslint.config.mjs` also covers the text props of the UI kit's components (`label`,
 * `description`, …): a kit component renders them as text, so a literal there is a word outside the locale.
 */
const eslint = new ESLint({ cwd: fileURLToPath(new URL("..", import.meta.url)) });

async function restricted(code: string): Promise<string[]> {
  const [result] = await eslint.lintText(code, { filePath: fileURLToPath(new URL("../../app/page.tsx", import.meta.url)) });
  return (result?.messages ?? []).filter((m) => m.ruleId === "no-restricted-syntax").map((m) => m.message);
}

describe("literal text in kit props", () => {
  it.each([
    '<Submit kind="primary" label="Save" busyLabel={t("a.b")} />',
    '<Head title="Welcome" />',
    '<Section id="x" title={t("a.b")} description={"Some words"} />',
    "<EmptyState icon=\"mail\" title={t(\"a.b\")} text={`Nothing here`} />",
    '<SwitchLine question="Already?" link={t("a.b")} href="/signin" />',
  ])("is refused: %s", async (element) => {
    expect(await restricted(`export const x = ${element};`)).not.toEqual([]);
  });

  it("allows words from the locale and identifiers", async () => {
    expect(await restricted('export const x = <TextField name="email" type="email" label={t("field.email")} />;')).toEqual([]);
  });
});
