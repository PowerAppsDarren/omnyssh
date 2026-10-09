import { expect, test, type Page } from '@playwright/test';

// Streamer mode (tech-gui.md §4.1, 2026-10-09 note). Tauri is stubbed at
// `__TAURI_INTERNALS__` (§6.4); `__fire` plays a core event, so an error carrying a
// real address can be sent the way the backend would send it.
const HOSTS = [
  {
    name: 'db',
    hostname: 'db.example.com',
    user: 'admin',
    port: 22,
    tags: [],
    source: 'manual',
    hasKey: true,
    monitoring: 'ssh',
    localForwards: [{ bindPort: 8080, remoteHost: '10.20.3.7', remotePort: 5432 }],
    tunnelAutostart: false,
    forwardAgent: false
  }
];

async function boot(page: Page, streamer: boolean): Promise<void> {
  await page.addInitScript(
    ({ hosts, streamer }) => {
      localStorage.setItem('omnyssh-streamer-mode', String(streamer));
      let cbid = 0;
      const listeners: Record<string, number[]> = {};
      const win = window as unknown as Record<string, unknown>;
      win.__fire = (event: string, payload: unknown): void => {
        for (const id of listeners[event] ?? []) {
          const cb = win[`__cb${id}`] as ((e: unknown) => void) | undefined;
          cb?.({ event, id, payload });
        }
      };
      (win as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
        invoke: (cmd: string, args: Record<string, unknown>) => {
          switch (cmd) {
            case 'list_hosts':
              return Promise.resolve(hosts);
            case 'plugin:event|listen': {
              const { event, handler } = args as { event: string; handler: number };
              (listeners[event] ||= []).push(handler);
              return Promise.resolve(cbid);
            }
            default:
              return Promise.resolve(null);
          }
        },
        transformCallback: (cb: unknown) => {
          const id = ++cbid;
          win[`__cb${id}`] = cb;
          return id;
        },
        unregisterCallback: (id: number) => {
          delete win[`__cb${id}`];
        }
      };
    },
    { hosts: HOSTS, streamer }
  );
  await page.goto('/');
  await expect(page.getByText('1 host', { exact: true })).toBeVisible();
}

const TUNNEL_ERROR = "Tunnel to 'db': 8080:10.20.3.7:5432 could not be opened: db.example.com refused";

async function fireError(page: Page, message: string): Promise<void> {
  await page.evaluate(
    (message) => (window as unknown as { __fire: (e: string, p: unknown) => void }).__fire('error', { message }),
    message
  );
}

test('streamer mode keeps real addresses out of the status bar and its tooltip', async ({ page }) => {
  await boot(page, true);
  await fireError(page, TUNNEL_ERROR);

  const status = page.getByRole('contentinfo').getByText("Tunnel to 'db'");
  await expect(status).toBeVisible();
  for (const real of ['10.20.3.7', 'db.example.com']) {
    await expect(status).not.toContainText(real);
    await expect(status).not.toHaveAttribute('title', new RegExp(real.replaceAll('.', '\\.')));
  }
  await expect(status).toContainText(':5432 could not be opened');
});

test('with streamer mode off the status bar shows the error as sent', async ({ page }) => {
  await boot(page, false);
  await fireError(page, TUNNEL_ERROR);
  await expect(page.getByRole('contentinfo').getByText(TUNNEL_ERROR)).toHaveAttribute('title', TUNNEL_ERROR);
});

test('a status-bar chip says streamer mode is on and turns it off', async ({ page }) => {
  await boot(page, true);
  await fireError(page, TUNNEL_ERROR);
  const statusBar = page.getByRole('contentinfo');
  const chip = statusBar.getByRole('button', { name: 'Streamer mode is on — click to turn it off' });
  await expect(chip).toHaveText('Streamer mode');
  await expect(chip).toHaveAttribute('title', /\(Ctrl\+Shift\+S\)$/);

  await chip.click();
  await expect(chip).toHaveCount(0);
  await expect(statusBar).toContainText(TUNNEL_ERROR);
  expect(await page.evaluate(() => localStorage.getItem('omnyssh-streamer-mode'))).toBe('false');
});

test('Ctrl+Shift+S toggles streamer mode and re-masks what is already on screen', async ({ page }) => {
  await boot(page, false);
  await fireError(page, TUNNEL_ERROR);
  const statusBar = page.getByRole('contentinfo');
  await expect(statusBar).toContainText('10.20.3.7');

  await page.keyboard.press('Control+Shift+S');
  await expect(statusBar).toContainText("Tunnel to 'db'");
  await expect(statusBar).not.toContainText('10.20.3.7');
  await expect(statusBar.getByRole('button', { name: /^Streamer mode is on/ })).toBeVisible();

  // Typing in a field does not hold it back.
  await page.getByRole('button', { name: 'Search hosts' }).click();
  await page.getByRole('textbox', { name: 'Search hosts' }).press('Control+Shift+S');
  await expect(statusBar).toContainText('10.20.3.7');
  await expect(statusBar.getByRole('button', { name: /^Streamer mode is on/ })).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: 'Search hosts' })).toHaveValue('');
});
