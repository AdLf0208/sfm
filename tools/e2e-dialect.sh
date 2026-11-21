#!/usr/bin/env bash
# In-game proof for the 26.3 dialect sweep (2026-11-21): one converted recipe
# crafts, and one converted loot table's block drops itself.
#
# A native `.hmod` cannot load on an unmodded game, so GAME must be a game the
# launcher built with this port. `ports/tools/launcher-game.sh build <mod>`
# produces that build and `... env <mod>` exports GAME/CTL/RES. The script is the
# same in every swept port: it derives the port from its own path and reads the
# port's `tools/dialect-proof.json`.
#
# The static leg runs first on the **delivered** `<mod>.hmod`, and on a pre-fix
# package when `HMOD=` names one (that red leg is how the checker is shown to be
# fail-capable).
#
# Fail-capable by construction:
#  * the craft fails when the recipe's ingredients are in the 1.21.1 object form
#    (the 26.3 parser rejects it and the result slot stays empty), and the grid
#    is read back to prove the ingredients were consumed;
#  * the drop fails when the pool uses `"conditions": [...]` (the block breaks
#    empty), and a vanilla crafting table broken beside it must drop one, so a
#    break/loot report that never works cannot pass;
#  * SELFTEST=1 inverts the loot assertion (the run MUST exit non-zero).
#
#   eval "$(ports/tools/launcher-game.sh env <mod>)"
#   ports/tools/launcher-game.sh run <mod>          # or BUILD_LOCAL=1 … for the slot
#   SELFTEST=1 GAME=… tools/e2e-dialect.sh
set -Eeuo pipefail
export LC_ALL=C

CLONE=${CLONE:-/home/adlf/Desktop/all_the_mods_10_rust_rewrite}
PORT=$(cd "$(dirname "$0")/.." && pwd)
MOD=$(basename "$PORT")
CTL=${CTL:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-ctl}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/$MOD-dialect.sock}
HMOD=${HMOD:-$PORT/$MOD.hmod}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
EVIDENCE=$CLONE/ports/_shared/evidence/$MOD
LEG=dialect
[ "$SELFTEST" = 1 ] && LEG=dialect-selftest
LOG=${LOG:-$EVIDENCE/e2e-$LEG.log}

mkdir -p "$EVIDENCE" "$(dirname "$LOG")"
exec > >(tee "$LOG") 2>&1

command -v jq >/dev/null || { echo "missing jq" >&2; exit 1; }
[ -n "$GAME" ] || {
    echo "GAME is required: the SDK package is built into a game by the launcher," >&2
    echo "  ports/tools/launcher-game.sh build $MOD  then  eval \"\$(ports/tools/launcher-game.sh env $MOD)\"" >&2
    exit 1
}
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -f "$HMOD" ] || { echo "missing package: $HMOD" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/$MOD-dialect-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"
cd "$SCRATCH"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 40 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR
cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    [ "$KEEP" = 1 ] || rm -rf "$SCRATCH" 2>/dev/null || true
    echo "EXIT:$status"
}
trap cleanup EXIT

echo "== static: every carried recipe and loot table is in the 26.3 dialect =="
if [ "${STATIC:-1}" = 1 ]; then
    python3 "$CLONE/ports/tools/check_261_dialect.py" "$HMOD" \
        || fail "a carried recipe or loot table is in the 1.21.1 dialect (see above)"
else
    echo "  skipped: STATIC=0 (a red leg whose package is known broken; see dialect-check-red.log)"
fi

echo "== launch (launcher-built game, Overworld arena) =="
"$CTL" --socket "$SOCK" launch --game "$GAME" --world "${WORLD:-Dialect$MOD}" \
    --seed 20261121 --creative --offscreen --wait --resources "$RES" >/dev/null
ctl wait-ready 120
ctl wait 40
[ "$(ctl state | jq -r '.screen')" = "playing" ] || fail "the game is not playing after launch"

AX=-8; AY=100; AZ=0
ctl command "gamemode creative @s" >/dev/null
ctl command "difficulty peaceful" >/dev/null
ctl command "weather clear" >/dev/null
ctl command "kill @e[type=!player]" >/dev/null
ctl fill $((AX - 2)) $((AY - 1)) $((AZ - 6)) $((AX + 18)) $((AY - 1)) $((AZ + 6)) minecraft:stone
ctl fill $((AX - 2)) $AY $((AZ - 6)) $((AX + 18)) $((AY + 5)) $((AZ + 6)) minecraft:air
ctl command "kill @e[type=!player]" >/dev/null
ctl tp "$AX" "$AY" "$AZ" >/dev/null
ctl wait 20
echo "  arena: stone floor at y=$((AY - 1)), table at $AX $AY $AZ, loot block at $((AX + 6)) $AY $AZ"

HOOK=(python3 "$CLONE/ports/tools/check_261_dialect.py" prove
      --sock "$SOCK" --ctl "$CTL" --spec "$PORT/tools/dialect-proof.json"
      --table "$AX" "$AY" "$AZ")
if [ "$SELFTEST" = 1 ]; then
    # The assertion is inverted on purpose (the expectation is impossible), so
    # the proof MUST fail. This exit 1 is the evidence that it can.
    echo "== SELFTEST: the assertion is inverted on purpose =="
    if "${HOOK[@]}" --selftest; then
        fail "SELFTEST: the in-game dialect proof passed when it must fail"
    fi
    echo "SELFTEST: the proof failed as required, so the green leg is not vacuous"
    echo "FAIL: SELFTEST expected the proof to fail and it did, so the green assertion can fail (this exit 1 is the proof)"
    exit 1
fi

"${HOOK[@]}" || fail "the in-game dialect proof failed (see the lines above)"

echo "PASS: $HMOD is in the 26.3 dialect and its in-game proof passed; the phases above name the recipe it crafted (or why it carries none) and the loot table it broke (or why none was in the 1.21.1 form), each with the vanilla control beside it"
exit 0
