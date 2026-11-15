import { test, expect, type Page } from '@playwright/test';
import { stubTauri } from './fixtures';

/**
 * Navigate to the app with the Tauri runtime stubbed via addInitScript so the
 * backend-less browser does not white-screen.
 */
async function gotoApp(page: Page): Promise<void> {
  await stubTauri(page);
  await page.goto('/');
}

test.describe('FlowLens smoke', () => {
  test('app loads without white-screen and brand/capture button are visible', async ({ page }) => {
    await gotoApp(page);

    // Header brand "流光 FlowLens" must be visible.
    await expect(page.locator('.logo-text')).toHaveText(/流光 FlowLens/);

    // Capture button must be visible (Header primary action).
    const captureBtn = page.locator('header button.btn.primary');
    await expect(captureBtn).toBeVisible();
    // The button renders either the start or stop label depending on `running`.
    await expect(captureBtn).toContainText(/开始捕获|停止捕获/);

    // The root container should exist (proves no white-screen / crash).
    await expect(page.locator('.app-container')).toBeVisible();
  });

  test('empty state is shown in sidebar and traffic table', async ({ page }) => {
    await gotoApp(page);

    // Sidebar empty state: "暂无网络流量"
    await expect(page.locator('.sidebar .empty-state')).toHaveText(/暂无网络流量/);

    // Traffic table empty state: "No traffic events found"
    await expect(page.locator('.traffic-table .empty-state')).toContainText('No traffic events found');

    // Filter bar match counter starts at 0 / 0.
    await expect(page.locator('.filter-count')).toContainText('0 / 0 匹配');
  });

  test('host filter typing updates the match counter', async ({ page }) => {
    await gotoApp(page);

    const hostInput = page.locator('#filter-host');
    await expect(hostInput).toBeVisible();

    // Typing a host filter must keep the "N / M 匹配" counter accurate (0 / 0
    // when there are no events, since the filter yields an empty set).
    await hostInput.fill('example.com');
    const counter = page.locator('.filter-count');
    await expect(counter).toContainText('0 / 0 匹配');

    // Clearing must also be reflected.
    await hostInput.fill('');
    await expect(counter).toContainText('0 / 0 匹配');
  });

  test('theme toggle changes data-theme on the html element', async ({ page }) => {
    await gotoApp(page);

    const toggleBtn = page.locator('header button.icon-btn');
    await expect(toggleBtn).toBeVisible();

    // The app applies the theme to <html data-theme="..."> once mounted.
    await expect.poll(() =>
      page.evaluate(() => document.documentElement.getAttribute('data-theme')),
    ).toMatch(/^(dark|light)$/);

    const before = await page.evaluate(() => document.documentElement.getAttribute('data-theme'));
    await toggleBtn.click();

    // Toggling must flip the attribute.
    await expect
      .poll(() => page.evaluate(() => document.documentElement.getAttribute('data-theme')))
      .not.toBe(before);

    const after = await page.evaluate(() => document.documentElement.getAttribute('data-theme'));
    expect(after === 'dark' || after === 'light').toBeTruthy();
  });

  test('settings modal opens and closes', async ({ page }) => {
    await gotoApp(page);

    // Settings trigger is the "⚙ 设置" button in the toolbar.
    const settingsTrigger = page.locator('button', { hasText: '设置' }).first();
    await expect(settingsTrigger).toBeVisible();

    // Modal should not be present initially.
    await expect(page.locator('.modal-overlay')).toHaveCount(0);

    // Open the modal.
    await settingsTrigger.click();
    await expect(page.locator('.modal-overlay')).toBeVisible();
    await expect(page.locator('.modal-content')).toBeVisible();

    // Close via the ✕ close button.
    await page.locator('.modal-header .close-btn').click();

    // Modal should disappear.
    await expect(page.locator('.modal-overlay')).toHaveCount(0);
  });

  test('no uncaught page errors occur during load and interactions', async ({ page }) => {
    const errors: unknown[] = [];
    page.on('pageerror', (err) => errors.push(err));

    await gotoApp(page);

    // Exercise interactions to surface runtime errors.
    await page.locator('header button.icon-btn').click();
    await page.locator('button', { hasText: '设置' }).first().click();
    await page.locator('.modal-header .close-btn').click();
    await page.locator('#filter-host').fill('example.com');

    // Let pending async handlers settle before asserting.
    await page.waitForTimeout(500);

    expect(errors, `Uncaught page errors:\n${errors.map((e) => String(e)).join('\n')}`).toEqual([]);
  });
});