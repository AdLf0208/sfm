#!/usr/bin/env bash
# In-game check for Super Factory Manager slice 1: a Factory Manager runs an
# SFML program over its cable network and moves items between chests.
#
# The End arena (stone floor at y=99, everything at y=100): an empty level keeps
# ambient item entities out of the container counts.
#
#  Rig  manager (2) - cable (3) - chest A (4, label "a") - cable (3,1) -
#       cable (3,2) - chest B (4,2, label "b"), and chest C (6, label "c") with
#       no cable beside it.
#
#  A  a manager placed from the world (`setblock`) has a block entity and
#     `/sfm show` reads state=no_program, triggers=0;
#  B  `/sfm label` puts a, b and c on the three chests; `/sfm labels` counts one
#     block each;
#  C  `EVERY 20 TICKS DO; INPUT FROM a; OUTPUT TO b; END` compiles
#     (state=running, triggers=1);
#  D  five diamonds in A move to B on the program's 20-tick timer, and
#     `/sfm show` reports the tick that moved them;
#  E  control: the same program with `OUTPUT TO c` (a label whose block has no
#     cable) moves nothing — upstream's "missing adjacent cable";
#  F  control: `EVERY 40 TICKS` does not fire in the first 25 ticks and does
#     later (the timer is the interval's, not every tick);
#  G  `INPUT 3 FROM a` moves exactly three of five and leaves two (a quantity);
#  H  control: `FORGET` between the input and the output moves nothing;
#  I  a program this slice cannot read (EMPTY SLOTS IN) is stored but reported
#     state=invalid with its line;
#  J  the manager, its program and its labels survive a relaunch.
#
# SELFTEST=1 flips D's assertion (the target must still be empty), so the
# scenario must FAIL; that proves the check can fail.
#
# Default run (EMBEDDED=1): the game is launched with **only the native bundle**
# (no `--dev-mod`) and the content comes from the bundle's embedded copy
# (`mixins/sfm/data/…`, the AE2 pattern). The run checks the file the helper
# wrote, and CONTROL=1 (the default with EMBEDDED) first launches the same
# binary with `.horizon-cache` unusable, where the ids must be unknown.
# EMBEDDED=0 loads `content/` as a dev mod instead.
#
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs
# the console path and bypasses the native `perform_command` hook.
set -euo pipefail
export LC_ALL=C

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/sfm
SHARED_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/sfm-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=SfmE2E
CONTROL_WORLD=SfmControl
# EMBEDDED=1 launches with no --dev-mod: the delivered .hnmod carries its content.
EMBEDDED=${EMBEDDED:-1}
CONTROL=${CONTROL:-$EMBEDDED}
# A mode argument, because the runner forwards the command and not arbitrary
# environment: `tools/e2e.sh selftest` (must fail), `dev` (content as a dev mod)
# or `embedded` (the default: one bundle, no --dev-mod).
case "${1:-}" in
    selftest) SELFTEST=1 EMBEDDED=0 CONTROL=0 ;;
    dev) EMBEDDED=0 ;;
    embedded) EMBEDDED=1 ;;
    "") ;;
    *) echo "usage: tools/e2e.sh [selftest|dev|embedded]" >&2; exit 2 ;;
esac
CONTROL=${CONTROL:-$EMBEDDED}
DEV_MOD=()
[ "$EMBEDDED" = 1 ] || DEV_MOD=(--dev-mod "$PORT/content")
MANAGER="2 100 0"
CHEST_A="4 100 0"
CHEST_B="4 100 2"
CHEST_C="6 100 0"

command -v jq >/dev/null || { echo "missing jq (every assertion parses a ctl reply)" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/sfm-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 80 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

# count X Y Z ITEM: how many of ITEM the container at X,Y,Z holds.
count() {
    ctl container "$1" "$2" "$3" \
        | jq -r --arg i "$4" '[.slots[] | select(.item == $i) | .count] | add // 0'
}
block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    if [ "$EMBEDDED" = 1 ]; then
        echo "  launch: no --dev-mod; the bundle carries its own content" >&2
    fi
    "$CTL" --socket "$SOCK" launch \
        --game "$GAME" --world "$world" --seed 20261105 --creative --headless --wait \
        --resources "$RES" "${DEV_MOD[@]}" >/dev/null
    ctl wait-ready 120 >/dev/null
    ctl wait 40
}

# Put the player back in the End and wait until the arena's chunk is loaded. A
# relaunch spawns them in the Overworld, where the manager's chunk is not loaded
# and its block entity does not tick.
arrive() {
    ctl command "execute in minecraft:the_end run tp @s 0.5 100 3.5"
    for _ in $(seq 1 120); do
        ctl wait 5
        if [ "$(ctl block 0 99 0 | jq -r '.loaded // false')" = true ]; then
            return
        fi
    done
    fail "the End arena chunk never loaded"
}

# A stone floor at y=99 in the End with clear air above, the player on it. It
# clears everything above the floor, so it runs before the rig is built and
# never after a relaunch (the rig stays).
arena() {
    arrive
    ctl command "execute in minecraft:the_end run fill -8 99 -8 12 99 8 minecraft:stone"
    ctl command "execute in minecraft:the_end run fill -8 100 -8 12 108 8 minecraft:air"
    ctl command "execute in minecraft:the_end run kill @e[type=!player]"
    ctl wait 20
}

block_at() { ctl command "execute in minecraft:the_end run setblock $1 100 $2 $3"; }

# `/sfm show X Y Z`: the state line, echoed, in SHOW (also prints the program
# error line when there is one).
show() { # show X Y Z
    say "sfm show $1 $2 $3"
    ctl wait 3 >/dev/null
    SHOW=$(ctl messages 8 | jq -r '.chat[]' | grep -m1 '^SFM pos=' || true)
    [ -n "$SHOW" ] || fail "no /sfm show reply for $1 $2 $3"
    echo "  $SHOW"
}
expect_show() { # expect_show KEY=VALUE
    case " $SHOW " in
        *" $1 "*) ;;
        *) fail "the state line has no '$1': $SHOW" ;;
    esac
}

# The program text for an `/sfm program` command: `;` is the line break, because
# a chat line cannot hold one.
set_program() { # set_program TEXT…
    say "sfm program $MANAGER $*"
    ctl wait 3 >/dev/null
}

# A program with a name and no trigger: `state=running`, `triggers=0`, so
# nothing moves while a phase fills a chest.
idle() { set_program "NAME \"idle\""; }

# Arm a program from a known tick: reset the counter while the idle program is
# still in place, then set the program, so the first firing is exactly one
# interval later and never lands in the setup.
arm() { # arm PROGRAM
    say "sfm tick $MANAGER 0"
    ctl wait 1 >/dev/null
    set_program "$1"
}

# The control for EMBEDDED=1: the same binary (every hook is in it) with the
# embedded content made unavailable — `.horizon-cache` is a plain file, so the
# helper can neither compare nor write its content file and falls back to
# `Continue`. Both ids must then be unknown; if they resolve, the run proves
# nothing about the embedded file.
control_leg() {
    echo "== control: the same game with no embedded content file =="
    rm -rf "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    launch "$CONTROL_WORLD"
    arena
    ctl command "item replace entity @s hotbar.0 with sfm:manager 1" >/dev/null || true
    ctl wait 4
    ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "sfm:manager")] | length == 0' >/dev/null \
        || fail "the control game has the manager item, so the content did not come from the embedded file"
    ctl command "execute in minecraft:the_end run setblock 40 100 0 sfm:manager" >/dev/null || true
    ctl wait 4
    local name
    name=$(block_name 40 100 0)
    [ "$name" = "minecraft:air" ] || fail "the control game holds $name, so the block id was known"
    [ -f "$SCRATCH/.horizon-cache" ] || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    echo "  without the embedded file: no manager item, the block stayed $name"
    ctl quit >/dev/null 2>&1 || true
    rm -f "$SCRATCH/.horizon-cache"
}

# EMBEDDED=1: the bundle must have written its content out at startup, byte for
# byte the delivered content mod.
embedded_content_check() {
    local cache=$SCRATCH/.horizon-cache/sfm-embedded-content.hmod
    [ -f "$cache" ] || fail "the bundle did not write its embedded content to $cache"
    cmp -s "$cache" "$PORT/content/sfm.hmod" \
        || fail "the embedded content differs from content/sfm.hmod"
    echo "  embedded content: $(wc -c < "$cache" | tr -d ' ') B at $cache, identical to content/sfm.hmod"
}

# The control binary: the shared one when it runs here (it is the pin's), else
# this port's own build inside the lease.
pick_ctl() {
    if [ -n "${CTL:-}" ] && [ -x "${CTL}" ] && "$CTL" hello >/dev/null 2>&1; then
        return
    fi
    if [ -x "$SHARED_BIN/horizon-ctl" ] && "$SHARED_BIN/horizon-ctl" hello >/dev/null 2>&1; then
        CTL=$SHARED_BIN/horizon-ctl
        return
    fi
    echo "  no usable horizon-ctl found; building this port's own" >&2
    python3 tools/horizon.py ctl >/dev/null
    CTL=$PORT/target/horizon-cargo/fast/horizon-ctl
    [ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
}
cd "$PORT"
pick_ctl
echo "  ctl: $CTL"

echo "== launch (The End arena, content + native bundle) =="
if [ "$CONTROL" = 1 ]; then control_leg; fi
launch
if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi
arena

echo "== the rig: manager 2, cable 3, chest A 4 (label a), cable 3:1, cable 3:2, chest B 4:2 (label b), chest C 6 (label c) =="
block_at 2 0 sfm:manager
block_at 3 0 sfm:cable
block_at 4 0 minecraft:chest
block_at 3 1 sfm:cable
block_at 3 2 sfm:cable
block_at 4 2 minecraft:chest
block_at 6 0 minecraft:chest
ctl wait 6
[ "$(block_name 2 100 0)" = "sfm:manager" ] || fail "the manager did not place"
[ "$(block_name 3 100 1)" = "sfm:cable" ] || fail "the second cable did not place"
[ "$(block_name 4 100 2)" = "minecraft:chest" ] || fail "chest B did not place"

echo "== A: a manager with no program yet =="
show 2 100 0
expect_show "state=no_program"
expect_show "triggers=0"

echo "== B: three labels on the manager =="
say "sfm label $MANAGER $CHEST_A a"
ctl wait 2 >/dev/null
say "sfm label $MANAGER $CHEST_B b"
ctl wait 2 >/dev/null
say "sfm label $MANAGER $CHEST_C c"
ctl wait 3 >/dev/null
show 2 100 0
expect_show "labels=3"
say "sfm labels $MANAGER"
ctl wait 3 >/dev/null
LABELS=$(ctl messages 10 | jq -r '.chat[]' | grep '^label ' || true)
echo "$LABELS" | sed 's/^/  /'
for name in a b c; do
    echo "$LABELS" | grep -q "label \"$name\": 1 block(s)" \
        || fail "the label list does not show $name once"
done

echo "== C: a program the slice runs =="
idle
set_program "NAME \"first\";EVERY 20 TICKS DO;INPUT FROM a;OUTPUT TO b;END"
show 2 100 0
expect_show "state=running"
expect_show "triggers=1"

echo "== D: five diamonds move from a to b on the 20-tick timer =="
idle
show 2 100 0
expect_show "triggers=0"
ctl container 4 100 0 minecraft:diamond 5 >/dev/null
ctl wait 4 >/dev/null
BEFORE_A=$(count 4 100 0 minecraft:diamond)
BEFORE_B=$(count 4 100 2 minecraft:diamond)
[ $((BEFORE_A + BEFORE_B)) = 5 ] || fail "the rig holds $((BEFORE_A + BEFORE_B)) diamonds, expected 5"
arm "EVERY 20 TICKS DO;INPUT FROM a;OUTPUT TO b;END"
ctl wait 30 >/dev/null
show 2 100 0
idle
AFTER_A=$(count 4 100 0 minecraft:diamond)
AFTER_B=$(count 4 100 2 minecraft:diamond)
echo "  a: $BEFORE_A -> $AFTER_A, b: $BEFORE_B -> $AFTER_B"
if [ "$SELFTEST" = 1 ]; then
    [ "$AFTER_B" = "$BEFORE_B" ] || fail "SELFTEST: chest B gained diamonds, so the scenario is alive"
else
    [ "$AFTER_A" = 0 ] || fail "chest A still holds $AFTER_A diamonds after the program tick"
    [ "$AFTER_B" = 5 ] || fail "chest B holds $AFTER_B diamonds, expected 5"
fi

echo "== D2: one timer tick moves its whole stack, and the state line counts it =="
idle
ctl container 4 100 0 minecraft:diamond 3 >/dev/null
ctl wait 4 >/dev/null
arm "EVERY 20 TICKS DO;INPUT FROM a;OUTPUT TO b;END"
ctl wait 25 >/dev/null
show 2 100 0
expect_show "moved=3"
expect_show "moves=1"
idle
echo "  a: $(count 4 100 0 minecraft:diamond), b: $(count 4 100 2 minecraft:diamond)"
[ "$(count 4 100 0 minecraft:diamond)" = 0 ] || fail "the second tick did not take the three diamonds"
[ "$(count 4 100 2 minecraft:diamond)" = 8 ] || fail "chest B holds $(count 4 100 2 minecraft:diamond) diamonds, expected 8"

echo "== E: control — a label with no cable beside it moves nothing =="
idle
ctl container 4 100 0 minecraft:stone 5 >/dev/null
ctl wait 4 >/dev/null
arm "EVERY 20 TICKS DO;INPUT FROM a;OUTPUT TO c;END"
ctl wait 40 >/dev/null
idle
echo "  a: $(count 4 100 0 minecraft:stone), c: $(count 6 100 0 minecraft:stone)"
[ "$(count 4 100 0 minecraft:stone)" = 5 ] || fail "stone left a chest whose target label has no cable"
[ "$(count 6 100 0 minecraft:stone)" = 0 ] || fail "the off-network chest took stone"

echo "== F: EVERY 40 TICKS does not fire in 25 ticks =="
arm "EVERY 40 TICKS DO;INPUT FROM a;OUTPUT TO b;END"
ctl wait 25 >/dev/null
EARLY_A=$(count 4 100 0 minecraft:stone)
EARLY_B=$(count 4 100 2 minecraft:stone)
echo "  after 25 ticks — a: $EARLY_A, b: $EARLY_B"
[ "$EARLY_A" = 5 ] || fail "a 40-tick program moved at 25 ticks ($EARLY_A stone left)"
[ "$EARLY_B" = 0 ] || fail "a 40-tick program moved at 25 ticks (b holds $EARLY_B)"
ctl wait 25 >/dev/null
idle
echo "  after 50 ticks — a: $(count 4 100 0 minecraft:stone), b: $(count 4 100 2 minecraft:stone)"
[ "$(count 4 100 0 minecraft:stone)" = 0 ] || fail "the 40-tick program did not fire by 50 ticks"
[ "$(count 4 100 2 minecraft:stone)" = 5 ] || fail "the 40-tick program moved the wrong amount"

echo "== G: INPUT 3 moves exactly three of five =="
idle
ctl container 4 100 0 minecraft:gold_ingot 5 >/dev/null
ctl wait 4 >/dev/null
arm "EVERY 20 TICKS DO;INPUT 3 FROM a;OUTPUT TO b;END"
# One firing only: the next is at tick 40.
ctl wait 25 >/dev/null
idle
echo "  a: $(count 4 100 0 minecraft:gold_ingot), b: $(count 4 100 2 minecraft:gold_ingot)"
[ "$(count 4 100 2 minecraft:gold_ingot)" = 3 ] || fail "a quantity of 3 did not move 3 gold"
[ "$(count 4 100 0 minecraft:gold_ingot)" = 2 ] || fail "a quantity of 3 took more than three gold"

echo "== H: control — FORGET between the input and the output moves nothing =="
idle
ctl container 4 100 0 minecraft:emerald 5 >/dev/null
ctl wait 4 >/dev/null
arm "EVERY 20 TICKS DO;INPUT FROM a;FORGET;OUTPUT TO b;END"
ctl wait 30 >/dev/null
idle
echo "  a: $(count 4 100 0 minecraft:emerald), b: $(count 4 100 2 minecraft:emerald)"
[ "$(count 4 100 0 minecraft:emerald)" = 5 ] || fail "FORGET still moved the emeralds"
[ "$(count 4 100 2 minecraft:emerald)" = 0 ] || fail "the target took emeralds after FORGET"

echo "== I: a program this slice cannot read is reported, not run =="
set_program "EVERY 20 TICKS DO;OUTPUT TO EMPTY SLOTS IN b;END"
show 2 100 0
expect_show "state=invalid"
ERROR_LINE=$(ctl messages 8 | jq -r '.chat[]' | grep -m1 '^program error: line' || true)
[ -n "$ERROR_LINE" ] || fail "no program error line for an unreadable program"
echo "  $ERROR_LINE"

echo "== J: the manager, its program and its labels survive a relaunch =="
set_program "EVERY 20 TICKS DO;INPUT FROM a;OUTPUT TO b;END"
ctl wait 4 >/dev/null
ctl quit >/dev/null
sleep 2
launch
arrive
ctl wait 10 >/dev/null
[ "$(block_name 2 100 0)" = "sfm:manager" ] || fail "the manager did not come back after the relaunch"
show 2 100 0
expect_show "state=running"
expect_show "triggers=1"
expect_show "labels=3"
idle
ctl container 4 100 0 minecraft:redstone 4 >/dev/null
ctl wait 4 >/dev/null
arm "EVERY 20 TICKS DO;INPUT FROM a;OUTPUT TO b;END"
ctl wait 30 >/dev/null
idle
echo "  after relaunch — a: $(count 4 100 0 minecraft:redstone), b: $(count 4 100 2 minecraft:redstone)"
[ "$(count 4 100 2 minecraft:redstone)" = 4 ] || fail "the program did not run after a relaunch"

echo "PASS: the manager ran EVERY 20 TICKS DO INPUT FROM a OUTPUT TO b and moved 5 diamonds; a label with no cable moved nothing; EVERY 40 TICKS did nothing at 25 ticks and fired by 50; INPUT 3 moved exactly 3 of 5; FORGET moved nothing; an unreadable program reported its line; and the program and labels survived a relaunch"
