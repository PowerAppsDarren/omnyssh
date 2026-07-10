// Thin typed wrappers over the generated command bindings (tech-gui.md §3.5).
// Components call these, never `invoke` directly.

import { commands } from '$lib/bindings';
import type { HostDto, SnippetDto } from '$lib/bindings';

export async function listHosts(): Promise<HostDto[]> {
  const res = await commands.listHosts();
  if (res.status === 'error') throw new Error(res.error.message);
  return res.data;
}

/** Reload hosts from disk and (re)start the pollers; broadcasts `hosts-loaded`. */
export async function reloadHosts(): Promise<void> {
  const res = await commands.reloadHosts();
  if (res.status === 'error') throw new Error(res.error.message);
}

/** Read the saved snippets from the shared `snippets.toml`. */
export async function listSnippets(): Promise<SnippetDto[]> {
  const res = await commands.listSnippets();
  if (res.status === 'error') throw new Error(res.error.message);
  return res.data;
}

/** Upsert one snippet by name and persist the whole list. */
export async function saveSnippet(snippet: SnippetDto): Promise<void> {
  const res = await commands.saveSnippet(snippet);
  if (res.status === 'error') throw new Error(res.error.message);
}

/** Delete the snippet named `name` and persist. */
export async function deleteSnippet(name: string): Promise<void> {
  const res = await commands.deleteSnippet(name);
  if (res.status === 'error') throw new Error(res.error.message);
}

/** Run a snippet on one or more hosts; results arrive as `snippet-result` events. */
export async function executeSnippet(
  snippetName: string,
  hostNames: string[],
  params: Record<string, string>
): Promise<void> {
  const res = await commands.executeSnippet(snippetName, hostNames, params);
  if (res.status === 'error') throw new Error(res.error.message);
}
