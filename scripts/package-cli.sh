#!/usr/bin/env bash
# Assembles the ready-to-use command-line package (CI artifact "ScrollRecord-CLI-<OS>").
#
# Why: lets anyone render videos with scrollrecord(.exe) without installing
#      Rust, Node or build tools: unzip, double-click render-example.bat.
# Usage: cargo build --release -p sr-cli && scripts/package-cli.sh
# Output: dist/ScrollRecord-CLI/
#           scrollrecord(.exe), render(.bat|.sh), render-example(.bat|.sh),
#           README.txt, licenses, example/lumen.srflow.json + example/site/
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/dist/ScrollRecord-CLI"

if [[ "${OS:-}" == "Windows_NT" ]]; then
  exe="scrollrecord.exe"
  launchers=(render.bat render-example.bat)
else
  exe="scrollrecord"
  launchers=(render.sh render-example.sh)
fi

if [[ ! -f "$root/target/release/$exe" ]]; then
  echo "error: $root/target/release/$exe not found. Run: cargo build --release -p sr-cli" >&2
  exit 1
fi

rm -rf "$out"
mkdir -p "$out/example/site"

# ── Program, launchers, docs ──
cp "$root/target/release/$exe" "$out/"
for f in "${launchers[@]}"; do
  cp "$root/packaging/cli/$f" "$out/"
done
cp "$root/packaging/cli/README.txt" "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$out/"
chmod +x "$out/$exe" "$out"/*.sh 2>/dev/null || true

# ── Example: the test landing page + its scenario, with "site" pointing to example/site ──
cp -R "$root/tests/fixtures/sites/01-plain-landing/." "$out/example/site/"
sed 's#"site": "[^"]*"#"site": "site"#' \
  "$root/examples/01-plain-landing.srflow.json" > "$out/example/lumen.srflow.json"
grep -q '"site": "site"' "$out/example/lumen.srflow.json" || {
  echo "error: could not point the example scenario at example/site" >&2
  exit 1
}

echo "packaged: $out"
ls -la "$out" "$out/example"
