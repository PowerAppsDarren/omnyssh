import { expect, test } from '@playwright/test';

// Collapse is manual and sticky across a reload (tech-gui.md §2, Stage 1.1). Off
// Tauri the choice rides the localStorage mirror the ui store keeps, so a plain
// browser reload is a faithful stand-in for an app restart.
test('sidebar collapse persists across reload', async ({ page }) => {
  await page.goto('/');

  const collapse = page.getByRole('button', { name: 'Collapse sidebar' });
  await expect(collapse).toBeVisible();
  await collapse.click();

  // Collapsing swaps the header affordance to Expand.
  await expect(page.getByRole('button', { name: 'Expand sidebar' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Collapse sidebar' })).toHaveCount(0);

  await page.reload();

  // Sticky: still collapsed after the reload.
  await expect(page.getByRole('button', { name: 'Expand sidebar' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Collapse sidebar' })).toHaveCount(0);
});
