#!/usr/bin/env bash
# Package both artifacts:
#  - the .hmod via the pin's own cargo-horizon-mod (build + check), from content/
#  - the source-only .hnmod (tools/package.py, deterministic bytes, in the port
#    root), carrying the built .hmod at mixins/<id>/data/ (the AE2
#    embedded-content pattern), so one file installs both halves.
# Run inside a build slot (it compiles the packaging tool):
#   ports/tools/build-slot.sh run --job md-pack --port ports/sfm \
#       --step package -- env CARGO_INCREMENTAL=0 tools/package.sh
set -euo pipefail

PORT=$(cd "$(dirname "$0")/.." && pwd)
HORIZON=${HORIZON:-${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}/ports/_shared/pins/horizon-1d63e0a}
LOGS=$PORT/target/ladder
mkdir -p "$LOGS"
cd "$PORT"

CARGO_TARGET_DIR=$PORT/target/carry-tools
export CARGO_TARGET_DIR
(cd content && cargo run --locked --offline --quiet \
    --manifest-path "$HORIZON/crates/cargo-horizon-mod/Cargo.toml" -- horizon-mod build) \
    2>&1 | tee "$LOGS/package-hmod.log"
cp content/target/horizon-mod/sfm.hmod content/sfm.hmod
(cd content && cargo run --locked --offline --quiet \
    --manifest-path "$HORIZON/crates/cargo-horizon-mod/Cargo.toml" -- horizon-mod check sfm.hmod) \
    2>&1 | tee -a "$LOGS/package-hmod.log"

# The bundle ships this build's content, never a stale copy: sync it in after the
# build, then let tools/package.py refuse anything that no longer matches.
mkdir -p mixins/sfm/data
cp content/sfm.hmod mixins/sfm/data/sfm-content.hmod

python3 tools/package.py | tee "$LOGS/package-hnmod.log"

(cd content && cargo run --locked --offline --quiet \
    --manifest-path "$HORIZON/crates/cargo-horizon-mod/Cargo.toml" -- horizon-mod check \
    ../sfm.hnmod --kind native-mod) \
    2>&1 | tee "$LOGS/check-native.log"

echo "== artifacts =="
for f in content/sfm.hmod sfm.hnmod; do
    [ -f "$f" ] || { echo "missing $f"; exit 1; }
    size=$(stat -c %s "$f")
    sha=$(sha256sum "$f" | cut -d' ' -f1)
    entries=$(python3 -c "import zipfile,sys; print(len(zipfile.ZipFile(sys.argv[1]).namelist()))" "$f")
    echo "$f size=$size sha256=$sha entries=$entries"
done
python3 -c "import zipfile; print('\n'.join(sorted(zipfile.ZipFile('sfm.hnmod').namelist())))"
