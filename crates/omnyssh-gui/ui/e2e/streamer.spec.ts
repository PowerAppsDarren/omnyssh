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

  const status = page.locator('footer').getByText("Tunnel to 'db'");
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
  await expect(page.locator('footer').getByText(TUNNEL_ERROR)).toHaveAttribute('title', TUNNEL_ERROR);
});
