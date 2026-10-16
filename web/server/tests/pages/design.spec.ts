import { expect, test, type Locator, type Page } from "@playwright/test";

import { reset, signIn } from "./support";

/**
 * Design verification 1 (`.tmp/verify-sa/result.md`): the findings a page can show, each checked where it can be
 * caught. Sizes are the frames' in `data/orchester2.pen`, with a pixel or two for font rendering.
 */

test.beforeEach(async () => {
  await reset();
});

async function box(locator: Locator) {
  const found = await locator.boundingBox();
  if (found === null) {
    throw new Error("not on the page");
  }
  return found;
}

async function hoverAndRead(page: Page, button: Locator, property: string): Promise<string> {
  await button.hover();
  return button.evaluate((element, name) => getComputedStyle(element).getPropertyValue(name), property);
}

test.describe("Projects on a phone (`mYBka`)", () => {
  test.use({ viewport: { width: 390, height: 787 } });

  test("M2: compact rows, the short callout and description, and the page as tall as drawn", async ({ page }) => {
    await signIn(page);
    const kontur = page.getByRole("link", { name: /^Kontur You own it/ });
    expect((await box(kontur)).height).toBeGreaterThan(85);
    expect((await box(kontur)).height).toBeLessThan(92);
    await expect(kontur.getByText("4 members · 3 open invites")).toBeVisible();
    await expect(kontur.getByText("github.com/acme/kontur")).toHaveCount(0);
    await expect(page.getByText("Each project has its own address on this server. People link their checkout to it in Orchester.")).toBeVisible();
    await expect(page.getByText("Orchester on oleh-pc, as Oleh Petrenko.")).toBeVisible();
    await expect(page.getByRole("link", { name: "Review in Devices" })).toBeVisible();
    // m2: the phone's own note.
    await expect(page.getByText("Tasks and runs are in Orchester on your computer; these pages run the server.")).toBeVisible();
    // The whole page fits the drawn 787: nothing below the window.
    expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBeLessThanOrEqual(790);
  });

  test("m1: the tabs sit right under the header, the title 24 below them", async ({ page }) => {
    await signIn(page);
    const tabs = await box(page.getByRole("navigation", { name: "Admin pages" }));
    expect(tabs.y).toBeLessThan(64);
    const title = await box(page.getByRole("heading", { name: "Projects", level: 1 }));
    expect(title.y - (tabs.y + tabs.height)).toBeGreaterThan(21);
    expect(title.y - (tabs.y + tabs.height)).toBeLessThan(27);
  });
});

test.describe("rows and columns (`FCQCr`, `YY8pR`, `iLFy5`)", () => {
  test("m3: project rows 88 high, their two right lines 19 apart; device rows 65 high", async ({ page }) => {
    await signIn(page);
    const kontur = page.getByRole("link", { name: /^Kontur You own it/ });
    const row = await box(kontur);
    expect(row.height).toBeGreaterThan(86);
    expect(row.height).toBeLessThan(91);
    const members = await box(kontur.getByText("4 members · 3 open invites"));
    const created = await box(kontur.getByText(/^Created /));
    expect(created.y - members.y).toBeGreaterThan(17);
    expect(created.y - members.y).toBeLessThan(22);
    await page.goto("/admin/devices");
    // A device row's height is the pitch of two rows in a row (`U7KMnL`).
    const joined = page.getByRole("region", { name: "Joined" });
    const first = await box(joined.getByText("Orchester on IVAN-PC"));
    const second = await box(joined.getByText("Orchester on alice-desktop"));
    expect(second.y - first.y).toBeGreaterThan(63);
    expect(second.y - first.y).toBeLessThan(67);
  });

  test("m4: a project's parts are 20 apart", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    const tabs = await box(page.getByRole("navigation", { name: "Project pages" }));
    const card = await box(page.getByRole("region", { name: "Invite by email" }));
    expect(card.y - (tabs.y + tabs.height)).toBeGreaterThan(18);
    expect(card.y - (tabs.y + tabs.height)).toBeLessThan(22);
  });

  test("m5: a revoked invite's line names the first name and keeps to one line", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    const line = page.getByText(/^Revoked by Alice, /);
    expect((await box(line)).height).toBeLessThan(20);
  });
});

test.describe("questions in place (`KagFB`, `WV4AT`)", () => {
  test("m6, m7: Revoke has the trash; the row's button stays plain and the question takes the focus", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    const open = page.getByRole("button", { name: "Revoke" }).first();
    await open.click();
    await expect(open).toHaveAttribute("aria-expanded", "true");
    expect(await hoverAndRead(page, open, "background-image")).toBe("none");
    const question = page.getByRole("group", { name: /^Revoke the invite/ });
    await expect(question.getByRole("button", { name: "Keep it" })).toBeFocused();
    await expect(question.getByRole("button", { name: "Revoke" }).locator("path[d='M3 6h18']")).toHaveCount(1);
  });

  test("m8: a refused Leave closes the question and leaves the row with its callout", async ({ page }) => {
    await signIn(page, "OWNR-0001");
    await page.goto("/admin/projects/mobile-app");
    await page.getByRole("button", { name: "Leave" }).click();
    await page.getByRole("group", { name: "Leave Mobile app?" }).getByRole("button", { name: "Leave" }).click();
    await expect(page.getByText("Mobile app needs an owner")).toBeVisible();
    await expect(page.getByRole("group", { name: "Leave Mobile app?" })).toHaveCount(0);
  });

  test("m9: the stale key list is a block above the cloud's card", async ({ page }) => {
    await reset("stale");
    await signIn(page);
    await page.goto("/admin/settings");
    const stale = await box(page.getByText("Orchester Cloud's keys here are out of date"));
    const link = await box(page.getByText("Link", { exact: true }));
    expect(stale.y).toBeLessThan(link.y);
  });

  test("m10: Unlink asks across the card", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/settings");
    await page.getByRole("button", { name: "Unlink…" }).click();
    const question = await box(page.getByRole("group", { name: "Unlink Kontur team from Orchester Cloud?" }));
    const section = await box(page.getByRole("region", { name: "Orchester Cloud" }));
    expect(question.width).toBeGreaterThan(section.width - 4);
  });

  test("m11, m12, m13: the waiting computer names the username; asking hides its buttons; approved keeps its line", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/devices");
    const waiting = page.getByRole("region", { name: "Waiting for approval" });
    await expect(waiting.getByText("Oleh Petrenko · oleh · joined through Orchester Cloud from 10.0.4.31", { exact: false })).toBeVisible();
    await waiting.getByRole("button", { name: "Refuse" }).click();
    const question = waiting.getByRole("group", { name: "Refuse Orchester on oleh-pc?" });
    await expect(waiting.getByRole("button", { name: "Approve" })).toHaveCount(0);
    await question.getByRole("button", { name: "Keep waiting" }).click();
    await waiting.getByRole("button", { name: "Approve" }).click();
    await expect(waiting.getByText("Approved by you")).toBeVisible();
    await expect(waiting.getByText("Oleh Petrenko · oleh · joined through", { exact: false })).toBeVisible();
    await expect(waiting.getByText("It joined as Oleh Petrenko and moves to Joined.", { exact: false })).toBeVisible();
  });

  test("m14: the admin's computers say yours, not which one this is", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/devices");
    await expect(page.getByText("Ivan Zaida · ivan · yours")).toHaveCount(2);
    await expect(page.getByText("this is you", { exact: false })).toHaveCount(0);
  });
});

test.describe("People (`waaNU`, `Gs9Vu`)", () => {
  test("m15, m16: server invites list with no heading; As an admin clears after sending", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/people");
    await expect(page.getByRole("heading", { name: "Invites to the server" })).toHaveCount(0);
    const admin = page.getByRole("checkbox", { name: "As an admin" });
    await admin.check();
    await page.getByLabel("Email", { exact: true }).fill("lena.new@example.com");
    await page.getByRole("button", { name: "Send invite" }).click();
    await expect(page.getByText("Invite sent to lena.new@example.com, as an admin")).toBeVisible();
    await expect(admin).not.toBeChecked();
  });
});

test.describe("sign-in and setup (`j8WujU`, `ulIoG`, `w7qUM`)", () => {
  test("m17, m18: a code is spaced out, its error icon on the first line; the cloud link plain at rest", async ({ page }) => {
    await page.goto("/admin/signin/code");
    const field = page.getByLabel("Code");
    await field.fill("H8RW-2NCF");
    expect(await field.evaluate((element) => getComputedStyle(element).letterSpacing)).toBe("2px");
    await page.getByRole("button", { name: "Sign in" }).click();
    const error = page.getByRole("alert").filter({ hasText: "That code isn't right" });
    const icon = await box(error.locator("svg"));
    const text = await box(error.locator("span"));
    expect(icon.y - text.y).toBeLessThan(5);
    const link = page.getByRole("link", { name: "Sign in with Orchester Cloud" });
    expect(await link.evaluate((element) => getComputedStyle(element).textDecorationLine)).toBe("none");
  });

  test("m22: at 390 the steps keep to one line and the address breaks only at its separators", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await reset("setup");
    await page.goto("/admin/setup");
    await page.getByLabel("Setup code").fill("Q4ZD-8M2K");
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByLabel("Server name").fill("Kontur team");
    await page.getByLabel("Username").fill("ivan");
    await page.getByLabel("Display name").fill("Ivan Zaida");
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByRole("link", { name: "Not now" }).click();
    const steps = await box(page.getByRole("list", { name: "Setup steps" }));
    expect(steps.height).toBeLessThan(30);
    expect(await page.locator("dd").first().innerHTML()).toContain("<wbr>");
  });
});

test.describe("Members (`Mhudf`, `KagFB`)", () => {
  test("m23: the browser's own select says what the chosen role means to assistive technology only", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur");
    const select = page.getByRole("combobox", { name: "Role of Oleh Petrenko" });
    await expect(select).toHaveAccessibleDescription("Works on Kontur's tasks and configuration.");
    await expect(page.getByText("Works on Kontur's tasks and configuration.").filter({ visible: true })).toHaveCount(0);
    await select.selectOption("owner");
    await expect(select).toHaveAccessibleDescription("Manages members and invites of Kontur.");
    await expect(page.getByText("Manages members and invites of Kontur.").filter({ visible: true })).toHaveCount(0);
  });

  test("R1: member rows keep their drawn 54 at 1440", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur");
    const first = await box(page.getByRole("combobox", { name: "Role of Ivan Zaida" }));
    const second = await box(page.getByRole("combobox", { name: "Role of Alice Moreau" }));
    expect(second.y - first.y).toBeGreaterThan(52);
    expect(second.y - first.y).toBeLessThan(57);
  });

  test("R1: at 390 the names keep to their line, whole, with · you", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await signIn(page);
    await page.goto("/admin/projects/kontur");
    const table = page.getByRole("table", { name: "Person" });
    await expect(table.getByText("· you", { exact: true })).toBeVisible();
    for (const name of ["Ivan Zaida", "Alice Moreau", "Oleh Petrenko"]) {
      const cell = table.getByText(name, { exact: true });
      expect((await box(cell)).height, name).toBeLessThan(20);
      expect(await cell.evaluate((element) => element.scrollWidth <= element.clientWidth), name).toBe(true);
    }
  });
});

test.describe("sign-in after a refusal (`j8WujU`)", () => {
  test("with join approval on, the code is the way on: it leads, and the cloud's button steps back", async ({ page }) => {
    await page.goto("/admin/signin?failed=approval_required");
    await expect(page.getByRole("link", { name: "Sign in with a code" })).toHaveClass(/primary/);
    await expect(page.getByRole("button", { name: "Sign in with Orchester Cloud" })).toHaveClass(/secondary/);
    await page.goto("/admin/signin");
    await expect(page.getByRole("button", { name: "Sign in with Orchester Cloud" })).toHaveClass(/primary/);
    await expect(page.getByRole("link", { name: "Sign in with a code" })).toHaveClass(/secondary/);
  });
});
