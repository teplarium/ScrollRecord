# sr-cli (`scrollrecord`)

Command-line access to ScrollRecord. Contains no business logic: it parses arguments, calls library crates and prints.

| Command | Phase | What it does |
|---|---|---|
| `scrollrecord info [--json]` | 0 | Versions and platform. Paste the output into bug reports |
| `scrollrecord errors [--json]` | 0 | Every `SR-xxxx` code with a summary |
| `scrollrecord render <flow> --target <id>` | 1 | Render a video (planned) |
| `scrollrecord validate <flow>` | 1 | Check a scenario file (planned) |
| `scrollrecord digest <site>` | 5 | Export a site digest for AI (planned) |
| `scrollrecord export-skills <dir>` | 5 | Write the AI skill packages (planned) |

Global flags: `--log-dir <DIR>` (also log to files), `-v` (debug logs).

```sh
cargo run -p sr-cli -- info
cargo test -p sr-cli
```
