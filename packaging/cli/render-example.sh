#!/usr/bin/env bash
# ScrollRecord: renders the included example landing page for desktop and iPhone.
here="$(cd "$(dirname "$0")" && pwd)"
echo "Rendering the example: desktop 16:9 and iPhone 9:16, 1080p, 60 fps. This takes a few minutes."
exec "$here/render.sh" "$here/example/lumen.srflow.json"
