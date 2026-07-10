import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// A component that hardcodes a brand hex is a theme bug (tech-gui.md §5.1). Token
// values live once in app.css / tailwind.config.ts (not components), so scanning
// .svelte files is the right net: every colour there must be a semantic token.
const SRC = fileURLToPath(new URL('../..', import.meta.url));
const HEX = /#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/;

function svelteFiles(): string[] {
  return readdirSync(SRC, { recursive: true, encoding: 'utf8' })
    .filter((f) => f.endsWith('.svelte'))
    .map((f) => join(SRC, f));
}

describe('no hardcoded brand hex in components', () => {
  it('every .svelte file styles from semantic tokens, not hex literals', () => {
    const files = svelteFiles();
    expect(files.length).toBeGreaterThan(0);

    const offenders: string[] = [];
    for (const file of files) {
      readFileSync(file, 'utf8')
        .split('\n')
        .forEach((line, i) => {
          if (HEX.test(line)) offenders.push(`${file}:${i + 1}  ${line.trim()}`);
        });
    }
    expect(offenders).toEqual([]);
  });
});
