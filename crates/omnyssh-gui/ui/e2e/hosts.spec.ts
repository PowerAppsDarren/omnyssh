import { expect, test, type Page } from '@playwright/test';

// Host management CRUD (tech-gui.md §4.1). e2e runs against the static SPA; Tauri is
// absent, so we stub `__TAURI_INTERNALS__` at the boundary (§6.4). The stub is
// stateful: save/delete mutate an in-memory list, and `reload_hosts` replays it as a
// `hosts-loaded` event through the same listener the app registers — so a save/delete
// round-trips into the dashboard grid exactly as the real backend would drive it.
const HOSTS = [
  { name: 'web-1', hostname: 'web-1.example.com', user: 'deploy', port: 22, tags: ['prod'], source: 'manual', hasKey: true, localForwards: [], tunnelAutostart: false, forwardAgent: false },
  { name: 'imported', hostname: 'imported.example.com', user: 'root', port: 22, tags: [], source: 'sshConfig', hasKey: false, localForwards: [], tunnelAutostart: false, forwardAgent: false },
  { name: 'api-1', hostname: 'api-1.example.com', user: 'deploy', port: 22, tags: ['prod', 'api'], source: 'manual', hasKey: true, localForwards: [], tunnelAutostart: false, forwardAgent: false }
];

async function boot(page: Page): Promise<void> {
  await page.addInitScript(
    ({ hosts }) => {
      let cbid = 0;
      const listeners: Record<string, number[]> = {};
      const state: { hosts: Array<Record<string, unknown>> } = { hosts: hosts.map((h) => ({ ...h })) };
      const win = window as unknown as Record<string, unknown>;

      function fire(event: string, payload: unknown): void {
        for (const id of listeners[event] ?? []) {
          const cb = win[`__cb${id}`] as ((e: unknown) => void) | undefined;
          cb?.({ event, id, payload });
        }
      }

      (win as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
        invoke: (cmd: string, args: Record<string, unknown>) => {
          switch (cmd) {
            case 'list_hosts':
              return Promise.resolve([...state.hosts]);
            case 'reload_hosts':
              // The real command reloads + restarts pollers, then broadcasts the list.
              setTimeout(() => fire('hosts-loaded', [...state.hosts]), 0);
              return Promise.resolve(null);
            case 'save_host': {
              // Upsert by name as a manual host; the outbound view (HostDto) omits the
              // secret fields the input carried, mirroring the backend map (§3.4).
              const h = args.input as Record<string, unknown> & { name: string; identityFile?: string };
              const view = {
                name: h.name,
                hostname: h.hostname,
                user: h.user,
                port: h.port,
                tags: (h.tags as string[]) ?? [],
                notes: h.notes,
                source: 'manual',
                hasKey: !!h.identityFile,
                localForwards: h.localForwards,
                tunnelAutostart: h.tunnelAutostart,
                forwardAgent: h.forwardAgent
              };
              const i = state.hosts.findIndex((x) => (x as { name: string }).name === view.name);
              if (i >= 0) state.hosts[i] = { ...state.hosts[i], ...view };
              else state.hosts.push(view);
              return Promise.resolve(null);
            }
            case 'delete_host':
              state.hosts = state.hosts.filter((x) => (x as { name: string }).name !== args.name);
              return Promise.resolve(null);
            case 'list_snippets':
              return Promise.resolve([]);
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
        }
      };
    },
    { hosts: HOSTS }
  );
  await page.goto('/');
  // The dashboard is the default screen; the seeded cards confirm the app booted.
  await expect(page.getByText('web-1', { exact: true })).toBeVisible();
}

test('adds a host and it appears as a card', async ({ page }) => {
  await boot(page);

  await page.getByRole('button', { name: 'Add host' }).click();
  const editor = page.getByRole('dialog', { name: 'Add host' });
  await expect(editor).toBeVisible();

  await editor.getByLabel('Name', { exact: true }).fill('db-1');
  await editor.getByLabel('Hostname / IP').fill('db-1.example.com');
  await editor.getByLabel('User').fill('postgres');
  await editor.getByRole('button', { name: 'Add host' }).click();

  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByText('db-1', { exact: true })).toBeVisible();
  await expect(page.getByText('postgres@db-1.example.com:22')).toBeVisible();
});

test('edits a manual host in place', async ({ page }) => {
  await boot(page);

  await page.getByRole('button', { name: 'Edit web-1' }).click();
  const editor = page.getByRole('dialog', { name: 'Edit host' });
  await expect(editor).toBeVisible();

  // The name is the on-disk key: fixed on edit.
  await expect(editor.getByLabel(/^Name/)).toHaveAttribute('readonly', '');

  const hostname = editor.getByLabel('Hostname / IP');
  await hostname.fill('web-1b.example.com');
  await editor.getByRole('button', { name: 'Save' }).click();

  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByText('deploy@web-1b.example.com:22')).toBeVisible();
});

test.describe('on Linux', () => {
  test.use({ userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)' });

  test('agent forwarding is off until switched on, and stays on', async ({ page }) => {
    await boot(page);

    await page.getByRole('button', { name: 'Edit web-1' }).click();
    let editor = page.getByRole('dialog', { name: 'Edit host' });
    const agent = editor.getByRole('switch', { name: 'Forward SSH agent' });
    await expect(agent).toHaveAttribute('aria-checked', 'false');
    await agent.click();
    await editor.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByRole('dialog')).toHaveCount(0);

    await page.getByRole('button', { name: 'Edit web-1' }).click();
    editor = page.getByRole('dialog', { name: 'Edit host' });
    await expect(editor.getByRole('switch', { name: 'Forward SSH agent' })).toHaveAttribute(
      'aria-checked',
      'true'
    );
  });
});

// The Desktop Chrome device reports a Windows user agent.
test('agent forwarding says it is not on Windows yet', async ({ page }) => {
  await boot(page);

  await page.getByRole('button', { name: 'Edit web-1' }).click();
  const editor = page.getByRole('dialog', { name: 'Edit host' });
  await expect(editor.getByRole('switch', { name: 'Forward SSH agent' })).toBeDisabled();
  await expect(editor.getByText('Not available on Windows yet.')).toBeVisible();
});

test('deletes a manual host after confirmation', async ({ page }) => {
  await boot(page);
  await expect(page.getByText('web-1', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Delete web-1' }).click();
  const confirm = page.getByRole('dialog', { name: 'Delete host' });
  await expect(confirm).toBeVisible();
  await confirm.getByRole('button', { name: 'Delete', exact: true }).click();

  await expect(page.getByText('web-1', { exact: true })).toHaveCount(0);
});

test('an SSH-config host is adopted by editing it', async ({ page }) => {
  await boot(page);

  // The import is marked, and there is nothing here to delete: it lives in
  // ~/.ssh/config, which this app never writes.
  await expect(page.getByText('ssh config')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Delete imported' })).toHaveCount(0);

  await page.getByRole('button', { name: 'Edit imported' }).click();
  const editor = page.getByRole('dialog', { name: 'Edit host' });
  await expect(editor).toBeVisible();
  // The form states what saving does, since the SSH config file itself does not change.
  await expect(editor.getByText(/never written/)).toBeVisible();

  await editor.getByLabel('Hostname / IP').fill('adopted.example.com');
  await editor.getByRole('button', { name: 'Save' }).click();

  // Saved as a manual copy: the import badge is gone and delete is now offered.
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByText('root@adopted.example.com:22')).toBeVisible();
  await expect(page.getByText('ssh config')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Delete imported' })).toHaveCount(1);
});

test('View options groups hosts under their first tag', async ({ page }) => {
  await boot(page);
  const trigger = page.getByRole('button', { name: 'View options' });
  const dot = trigger.locator('span.bg-accent');
  await expect(dot).toHaveCount(0);

  await trigger.focus();
  await page.keyboard.press('Enter');
  const panel = page.getByRole('dialog', { name: 'View options' });
  const grouping = panel.getByRole('switch', { name: 'Group by tag' });
  await expect(grouping).toBeFocused();
  await page.keyboard.press('Space');
  await expect(grouping).toHaveAttribute('aria-checked', 'true');
  await expect(dot).toHaveCount(1);

  // api-1 is tagged "prod, api": one card, under its first tag only.
  const prod = page.getByRole('region', { name: 'prod hosts' });
  await expect(prod.getByText('api-1', { exact: true })).toBeVisible();
  await expect(prod.getByText('web-1', { exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'api hosts' })).toHaveCount(0);
  await expect(page.getByText('api-1', { exact: true })).toHaveCount(1);
  await expect(page.getByRole('heading', { level: 2 })).toHaveText([/prod/, /Untagged/]);

  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(trigger).toBeFocused();

  // A press outside closes it too.
  await trigger.click();
  await expect(panel).toBeVisible();
  await page.getByRole('heading', { name: 'Dashboard' }).click();
  await expect(panel).toHaveCount(0);

  // `g` switches grouping back off.
  await page.keyboard.press('g');
  await expect(prod).toHaveCount(0);
  await expect(dot).toHaveCount(0);
});

test('folded sections stay folded across navigation and reload', async ({ page }) => {
  await boot(page);
  await page.keyboard.press('g');
  const header = () =>
    page.getByRole('region', { name: 'prod hosts' }).getByRole('button', { name: /^prod/ });

  await header().click();
  await expect(header()).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByText('web-1', { exact: true })).toBeHidden();
  await expect(page.getByText('imported', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Snippets', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Snippets' })).toBeVisible();
  await page.getByRole('button', { name: 'Dashboard', exact: true }).click();
  await expect(header()).toHaveAttribute('aria-expanded', 'false');

  await page.reload();
  await expect(header()).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByText('web-1', { exact: true })).toBeHidden();

  await header().focus();
  await page.keyboard.press('Enter');
  await expect(header()).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByText('web-1', { exact: true })).toBeVisible();
});

test('a host with a repeated tag renders once', async ({ page }) => {
  const errors: Error[] = [];
  page.on('pageerror', (e) => errors.push(e));
  await boot(page);

  await page.getByRole('button', { name: 'Edit web-1' }).click();
  const editor = page.getByRole('dialog', { name: 'Edit host' });
  await expect(editor.getByText('The first tag groups the host on the dashboard.')).toBeVisible();
  await editor.getByLabel('Tags').fill('prod, prod');
  await editor.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);

  await page.keyboard.press('g');
  const prod = page.getByRole('region', { name: 'prod hosts' });
  await expect(prod.getByText('web-1', { exact: true })).toHaveCount(1);
  await expect(prod.getByText('api-1', { exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});

test('g is ignored under a modal and while typing', async ({ page }) => {
  await boot(page);
  const grouped = page.getByRole('region', { name: 'Untagged hosts' });

  // The Support dialog is not the dashboard's own: g must not regroup the grid behind it.
  await page.getByRole('button', { name: 'Support OmnySSH' }).click();
  await expect(page.getByRole('dialog', { name: 'Support OmnySSH' })).toBeVisible();
  await page.keyboard.press('g');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);

  await page.getByRole('button', { name: 'Search hosts' }).click();
  await page.getByRole('textbox', { name: 'Search hosts' }).press('g');
  await expect(page.getByRole('textbox', { name: 'Search hosts' })).toHaveValue('g');
  await expect(grouped).toHaveCount(0);

  // Away from both, the same key groups.
  await page.getByRole('textbox', { name: 'Search hosts' }).press('Escape');
  await page.keyboard.press('g');
  await expect(grouped).toBeVisible();
});

test('rejects a new host whose name already exists', async ({ page }) => {
  await boot(page);

  await page.getByRole('button', { name: 'Add host' }).click();
  const editor = page.getByRole('dialog', { name: 'Add host' });
  await editor.getByLabel('Name', { exact: true }).fill('web-1');
  await editor.getByLabel('Hostname / IP').fill('dupe.example.com');
  await editor.getByRole('button', { name: 'Add host' }).click();

  // The editor stays open with an inline error rather than clobbering the existing host.
  await expect(editor).toBeVisible();
  await expect(editor.getByText('A host named "web-1" already exists')).toBeVisible();
});
