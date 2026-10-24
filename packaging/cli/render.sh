#!/usr/bin/env bash
# ScrollRecord: render a scenario file. Usage: ./render.sh path/to/scenario.srflow.json
# The videos are written to a "renders" folder next to the scenario file.
set -u
here="$(cd "$(dirname "$0")" && pwd)"
if [ $# -lt 1 ]; then
  echo "Usage: ./render.sh path/to/scenario.srflow.json"
  exit 1
fi
"$here/scrollrecord" render "$@"
status=$?
if [ $status -eq 0 ]; then
  echo "Done. The videos are in the \"renders\" folder next to the scenario."
else
  echo "Rendering failed. Look up the SR code above in docs/ERRORS.md on GitHub."
fi
exit $status
