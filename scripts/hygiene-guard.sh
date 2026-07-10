#!/usr/bin/env bash
# Repo hygiene guard: fail if an instruction/design doc, a private key, or an
# environment file is tracked in git. Scans the tracked tree of the current repo
# (git ls-files), so `git add`-ing a forbidden file turns this red; a clean tree
# is green. See tech-gui.md §0.0 / §11.5 and CLAUDE.md for the rule set.
set -eo pipefail

# Instruction/design docs that must never be committed (§11.5).
docs="tech-gui.md CLAUDE.md voice-brandbook.html instruction.md DISTRIBUTION.md"
# Marker for embedded private-key material (BRE).
key_marker='-----BEGIN [A-Z ]*PRIVATE KEY-----'

violations=()

while IFS= read -r path; do
  base=${path##*/}

  for d in $docs; do
    if [ "$base" = "$d" ]; then
      violations+=("instruction/design doc: $path")
    fi
  done

  case $base in
    .env | .env.*) violations+=("environment file: $path") ;;
  esac

  case $path in
    *.tauri.key) violations+=("updater signing key: $path") ;;
  esac

  # Content scan for private keys; skip this guard's own sources, which must
  # spell the marker out.
  case $path in
    scripts/hygiene-guard.sh | scripts/hygiene-guard.test.sh) ;;
    *)
      if [ -f "$path" ] && LC_ALL=C grep -Iq -e "$key_marker" -- "$path" 2>/dev/null; then
        violations+=("embedded private key: $path")
      fi
      ;;
  esac
done < <(git ls-files)

if [ "${#violations[@]}" -gt 0 ]; then
  echo "Repo hygiene guard failed — forbidden files tracked:" >&2
  printf '  - %s\n' "${violations[@]}" >&2
  exit 1
fi

echo "Repo hygiene guard passed: no forbidden files tracked."
