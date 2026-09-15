import { expect, test, type Page } from "@playwright/test";

async function openDeterministicAuthShell(page: Page, theme: "light" | "dark" = "light") {
  await page.addInitScript(({ selectedTheme }) => {
    localStorage.clear();
    localStorage.setItem("aro-theme", selectedTheme);
    Math.random = () => 0.25;
  }, { selectedTheme: theme });
  await page.route("**/v1/**", async (route) => {
    await route.fulfill({
      status: 401,
      contentType: "application/json",
      body: JSON.stringify({ error: "deterministic unauthenticated test session" }),
    });
  });
  await page.goto("/");
  await expect(page.locator(".splash-screen")).toBeHidden({ timeout: 15_000 });
  await expect(page.locator(".cloud-auth-page")).toBeVisible();
}

test.describe("unauthenticated web shell", () => {
  test("loads the extracted CSS files in their declared order", async ({ page }) => {
    await openDeterministicAuthShell(page);
    const styleIds = await page.locator("style[data-vite-dev-id]").evaluateAll((styles) =>
      styles.map((style) => style.getAttribute("data-vite-dev-id") ?? ""),
    );
    const expected = [
      "01-previews-spotlight-shell.css",
      "02-voice-core.css",
      "03-settings-dashboard-organization.css",
      "04-modals-markdown-chat.css",
      "05-memory-settings-arena.css",
      "06-splash-auth-command.css",
      "07-responsive-agent.css",
    ];
    const positions = expected.map((name) => styleIds.findIndex((id) => id.endsWith(name)));
    expect(positions.every((position) => position >= 0)).toBe(true);
    expect(positions).toEqual([...positions].sort((left, right) => left - right));
    await expect(page.locator(".cloud-auth-page")).toHaveCSS("position", "relative");
  });

  test("keeps the login layout visually stable without a backend", async ({ page }) => {
    await openDeterministicAuthShell(page);

    await expect(page.locator(".cloud-auth-page")).toHaveScreenshot("login-light.png", {
      animations: "disabled",
      caret: "hide",
      scale: "css",
    });
  });

  test("keeps the dark and mobile login layouts visually stable", async ({ page }) => {
    test.setTimeout(60_000);
    await openDeterministicAuthShell(page, "dark");
    await expect(page.locator(".cloud-auth-page")).toHaveScreenshot("login-dark.png", {
      animations: "disabled",
      caret: "hide",
      scale: "css",
    });

    await page.setViewportSize({ width: 390, height: 844 });
    await page.reload();
    await expect(page.locator(".splash-screen")).toBeHidden({ timeout: 15_000 });
    await expect(page.locator(".cloud-auth-page")).toHaveScreenshot("login-mobile-dark.png", {
      animations: "disabled",
      caret: "hide",
      scale: "css",
    });
  });

  test("switches between login and registration locally", async ({ page }) => {
    await openDeterministicAuthShell(page);

    const authPanel = page.locator(".cloud-auth-panel");
    await expect(authPanel.locator("input")).toHaveCount(2);

    await authPanel.getByRole("button", { name: /compte/i }).click();
    await expect(authPanel.locator("input")).toHaveCount(4);

    await authPanel.getByRole("button", { name: /connecter/i }).click();
    await expect(authPanel.locator("input")).toHaveCount(2);
  });
});
