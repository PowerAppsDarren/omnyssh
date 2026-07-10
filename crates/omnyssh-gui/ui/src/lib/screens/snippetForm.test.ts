import { describe, expect, it } from 'vitest';
import type { SnippetDto } from '$lib/bindings';
import {
  declaredParams,
  emptyForm,
  filterSnippets,
  formFromSnippet,
  formToSnippet,
  type SnippetFormFields
} from './snippetForm';

function fields(partial: Partial<SnippetFormFields>): SnippetFormFields {
  return { ...emptyForm(), ...partial };
}

function snippet(partial: Partial<SnippetDto>): SnippetDto {
  return { name: 'n', command: 'ls', scope: 'global', ...partial };
}

describe('formToSnippet — mirrors the TUI to_snippet', () => {
  it('rejects an empty name', () => {
    const r = formToSnippet(fields({ name: '  ', command: 'ls' }));
    expect(r).toEqual({ ok: false, error: 'Name cannot be empty' });
  });

  it('rejects an empty command', () => {
    const r = formToSnippet(fields({ name: 'n', command: '   ' }));
    expect(r).toEqual({ ok: false, error: 'Command cannot be empty' });
  });

  it('requires a host when scope is host', () => {
    const r = formToSnippet(fields({ name: 'n', command: 'ls', scope: 'host', host: '' }));
    expect(r).toEqual({ ok: false, error: "Host is required when scope is 'host'" });
  });

  it('keeps a host provided under global scope (matches the TUI)', () => {
    const r = formToSnippet(fields({ name: 'n', command: 'ls', scope: 'global', host: 'web' }));
    expect(r.ok && r.snippet.host).toBe('web');
  });

  it('trims and splits tags and params, dropping blanks', () => {
    const r = formToSnippet(fields({ name: 'n', command: 'ls', tags: 'a, b ,,', params: 'p1, p2' }));
    expect(r.ok && r.snippet.tags).toEqual(['a', 'b']);
    expect(r.ok && r.snippet.params).toEqual(['p1', 'p2']);
  });

  it('drops empty tags/params to undefined so the wire form stays sparse', () => {
    const r = formToSnippet(fields({ name: 'n', command: 'ls', tags: ' , ', params: '' }));
    expect(r.ok && r.snippet.tags).toBeUndefined();
    expect(r.ok && r.snippet.params).toBeUndefined();
  });

  it('trims the name and command', () => {
    const r = formToSnippet(fields({ name: '  deploy ', command: '  ls -la  ' }));
    expect(r.ok && r.snippet.name).toBe('deploy');
    expect(r.ok && r.snippet.command).toBe('ls -la');
  });
});

describe('formFromSnippet round-trips through formToSnippet', () => {
  it('reconstructs the same snippet from its form fields', () => {
    const original = snippet({
      name: 'restart',
      command: 'systemctl restart {{svc}}',
      scope: 'host',
      host: 'web-1',
      tags: ['ops', 'deploy'],
      params: ['svc']
    });
    const r = formToSnippet(formFromSnippet(original));
    expect(r).toEqual({ ok: true, snippet: original });
  });
});

describe('declaredParams', () => {
  it('returns the declared params or an empty list', () => {
    expect(declaredParams(snippet({ params: ['a', 'b'] }))).toEqual(['a', 'b']);
    expect(declaredParams(snippet({ params: undefined }))).toEqual([]);
  });
});

describe('filterSnippets — mirrors the TUI filter_snippets', () => {
  const list: SnippetDto[] = [
    snippet({ name: 'deploy', command: 'git pull', tags: ['ops'] }),
    snippet({ name: 'backup', command: 'pg_dump db', tags: ['db'] }),
    snippet({ name: 'restart', command: 'systemctl restart nginx' })
  ];

  it('returns everything for an empty query, order preserved', () => {
    expect(filterSnippets(list, '').map((s) => s.name)).toEqual(['deploy', 'backup', 'restart']);
  });

  it('matches on name', () => {
    expect(filterSnippets(list, 'depl').map((s) => s.name)).toEqual(['deploy']);
  });

  it('matches on command', () => {
    expect(filterSnippets(list, 'systemctl').map((s) => s.name)).toEqual(['restart']);
  });

  it('matches on a tag', () => {
    expect(filterSnippets(list, 'db').map((s) => s.name)).toEqual(['backup']);
  });

  it('is case-insensitive', () => {
    expect(filterSnippets(list, 'DEPLOY').map((s) => s.name)).toEqual(['deploy']);
  });

  it('returns nothing when there is no match', () => {
    expect(filterSnippets(list, 'zzz')).toEqual([]);
  });
});
