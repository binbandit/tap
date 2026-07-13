# tap

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-blue.svg)](https://www.rust-lang.org)

**Make paths exist.** `tap` is a modern, friendly replacement for `touch`:
create files and directories (parents included), seed content safely, set
permissions and timestamps — with output built for humans and JSON built for
scripts.

```console
$ tap src/components/{Button,Input,Modal}.tsx
✓ created      src/components/Button.tsx (parents created)
✓ created      src/components/Input.tsx
✓ created      src/components/Modal.tsx
```

## Why tap?

`touch` is fine until it isn't: no parent directories, no directories at all,
cryptic timestamp syntax, and nothing for the thing you actually do next
(put content in the file, make it executable, open it). `tap` keeps the muscle
memory — `-a`, `-m`, `-c`, `-r` mean what they've meant for 40 years — and
fixes the rest:

- **Parents are created for you.** `tap deep/nested/file.txt` just works
  (`--no-parents` restores classic behaviour).
- **Directories are first-class.** A trailing slash (`tap build/`) or `-d`
  makes directories instead of files.
- **Brace expansion on every shell.** `tap src/{lib,main}.rs`,
  `tap shot_{01..12}.png`, `tap config.json{,.bak}` — identical on bash,
  PowerShell, and cmd.
- **Safe by default.** `tap -w` seeds new files but *refuses to overwrite
  existing content* unless you say `--append` or `--force`.
- **Human timestamps.** `-t yesterday`, `-t -2h`, `-t '2 hours ago'`,
  `-t 2024-01-01`, `-t @1700000000` — interpreted in your local timezone.
- **It finishes the job.** `-x` makes a script executable (new scripts get a
  fitting shebang), `-e` opens the file in your `$EDITOR`.
- **Script-friendly.** `--json` for structured results, `--check` for
  existence tests with meaningful exit codes, `--dry-run` to preview.

## Installation

```bash
cargo install tap
```

Or build from source:

```bash
git clone https://github.com/crazywolf132/tap.git
cd tap
cargo build --release   # binary at target/release/tap
```

Shell completions: `tap --completions bash|zsh|fish|powershell|elvish`.

## Usage

### Make things exist

```bash
tap notes.md                       # create a file, or refresh its times
tap deep/nested/file.txt           # parents created automatically
tap build/ logs/                   # trailing slash = directory
tap -d cache tmp                   # or use -d
tap src/{models,views,tests}/mod.rs  # brace expansion
tap img_{01..12}.png               # numeric ranges (zero-padded)
tap -c *.log                       # touch existing only, create nothing
tap -n a/b/c.txt                   # dry run: prints what would happen
```

### Seed content — safely

```bash
tap -w 'node_modules/' .gitignore   # write content into a NEW file
tap --append -w 'dist/' .gitignore  # append a line (newlines handled)
tap --force -w 'fresh' notes.txt    # explicit consent to replace content
tap --template LICENSE-MIT LICENSE  # copy a template file's contents
```

`--write` adds a trailing newline if you didn't provide one, and `--append`
makes sure the appended text starts on its own line. If the target already
has content, `tap` stops and tells you your options instead of eating it:

```console
$ tap -w 'oops' .gitignore
✗ .gitignore: refusing to overwrite existing content
  hint: use --append to add to it, or --force to replace it
```

### Scripts, ready to run

```bash
tap -x deploy.sh      # executable, seeded with '#!/usr/bin/env bash'
tap -x tool.py        # '#!/usr/bin/env python3'
tap -e TODO.md        # create it, then open it in $VISUAL/$EDITOR
```

Shebangs are only added to brand-new empty files; `-x` on an existing file
just sets the executable bits.

### Times

```bash
tap file.txt                        # refresh atime+mtime to now (touch)
tap -t yesterday photo.jpg          # friendly words: now/today/yesterday/tomorrow
tap -t -2h report.txt               # relative: -2h, +30m, '3 days ago', 'in 1 week'
tap -t '2024-01-01 09:30' a.log     # ISO-ish, local timezone
tap -t @1700000000 epoch.txt        # unix epoch
tap -r original.txt copy.txt        # copy times from a reference file
tap -m -t yesterday build.log       # -a / -m limit it to atime / mtime
```

Ambiguous formats like `05/06/2024` are deliberately rejected rather than
guessed.

### Permissions

```bash
tap --mode 600 secrets.env          # octal modes (000-7777)
tap -R --mode 755 scripts/          # recursive over a directory
```

### For scripts

```bash
tap --check config.yml data/        # exit 0 if all exist, 1 otherwise
tap --json src/*.rs | jq '.results[].action'
```

JSON output is a single object:

```json
{
  "succeeded": 2,
  "failed": 0,
  "results": [
    { "path": "a.txt", "ok": true, "action": "created", "kind": "file",
      "changes": ["parents created"] }
  ]
}
```

`action` is one of `created`, `updated`, `touched`, `exists`, `missing`,
`skipped`, `would_create`, `would_update`, `would_touch`, `error`. Failed
paths carry `error` and often a `hint`.

### Output rules

- Interactive terminal: one tidy line per path (respects `NO_COLOR`).
- Piped: silent on success, errors on stderr — classic Unix.
- `-v` narrates even when piped; `-q` silences success output entirely.
- Exit codes: `0` all good, `1` some path failed (or `--check` miss),
  `2` usage error.

## Options

| Flag | Meaning |
| --- | --- |
| `-d, --dir` | treat all paths as directories |
| `-c, --no-create` | only update paths that already exist |
| `--no-parents` | fail on missing parent dirs (classic `touch`) |
| `-w, --write <TEXT>` | write TEXT (newline-terminated) into the file |
| `--template <FILE>` | copy FILE's contents into the target |
| `--append` | append instead of write (with `-w`/`--template`) |
| `--force` | allow replacing existing content |
| `-t, --at <WHEN>` | set times (`yesterday`, `-2h`, `2024-01-01`, `@epoch`, ...) |
| `-r, --reference <FILE>` | copy times from FILE |
| `-a, --atime` / `-m, --mtime` | limit time changes to atime / mtime |
| `--mode <OCTAL>` | set permissions, `-R` recurses into directories |
| `-x, --exec` | make executable; new scripts get a shebang |
| `--check` | report existence, change nothing, exit 1 on misses |
| `-n, --dry-run` | preview without touching the filesystem |
| `-e, --edit` | open touched files in `$VISUAL`/`$EDITOR` |
| `-q` / `-v` / `--json` | quiet / verbose / JSON output |
| `--completions <SHELL>` | print shell completions |

## Migrating from tap 1.x

tap 2.0 is a ground-up redesign. The breaking changes:

- **`-w` no longer truncates existing files.** Add `--force` to keep old
  behaviour, or `--append` for what you probably wanted. `-w` content is now
  newline-terminated.
- **`-a` and `-m` are `touch`'s atime/mtime selectors again.** Appending is
  `--append`; `-c` is now `--no-create` (also from `touch`).
- **`--chmod` is `--mode`** (the alias `--chmod` still works).
- **`--check` exits 1 when files are missing** (it always exited 0 before,
  which made it useless in scripts).
- **`-t` times are local**, not silently UTC, and both atime+mtime are set.
  Ambiguous `dd/mm` vs `mm/dd` formats are rejected instead of guessed.
- **Unmatched glob patterns are errors** instead of silently creating a file
  literally named `*.txt`.
- **`--trim`, `--line-endings`, `--encoding`, and YAML output were removed.**
  They belonged to other tools (`dos2unix`, `iconv`) and made every other
  flag noisier. `--output-format json` is now just `--json`.

## Architecture

- `cli` — the clap interface, grouped help, completions
- `expand` — brace expansion and glob matching
- `ops` — the per-path engine (create, content, safety rails)
- `times` — friendly timestamp parsing (local-time), atime/mtime application
- `mode` — octal permissions and the executable bit
- `report` — result model, human renderer, JSON renderer
- `lib::run` — pure orchestration: takes a `Cli`, returns a `Report`, prints
  nothing (that's what makes the whole tool testable end-to-end)

## Contributing

Contributions are welcome — `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check` should pass.

## License

MIT — see [LICENSE](LICENSE).
