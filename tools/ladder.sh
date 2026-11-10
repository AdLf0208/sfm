#!/usr/bin/env bash
# One ladder step in one lease: the runner deletes `stage/modded` before every
# stage/check/test/build step, and `tools/horizon.py` re-exports only when the
# stage signature changes, so the stage has to run first in the same lease.
#
#   ports/tools/build-slot.sh run --job sfm-build --port ports/sfm --step build \
#       -- env CARGO_INCREMENTAL=0 tools/ladder.sh build
set -Eeuo pipefail
PORT=$(cd "$(dirname "$0")/.." && pwd)
cd "$PORT"
export CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-0}
action=${1:-check}
python3 tools/horizon.py stage
python3 tools/horizon.py "$action"
