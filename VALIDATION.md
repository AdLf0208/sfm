# VALIDATION — sfm slice 1

> **Historical for 0.2.0.** This file records the retired `.hnmod` ladder
> (`tools/horizon.py stage → check → test → build`, the embedded content and the
> `.hnmod` packaging), which no longer exists. The 0.2.0 SDK ladder —
> `cargo test` / `clippy` / `fmt`, `cargo horizon-mod build` twice, `check`, `fits` —
> is in `notes.md`, "Migration to the SDK format", and in
> `../_shared/evidence/sfm/sdk-*.log`. No 0.2.0 in-game run exists
> (`horizon-launcher` is not installed here).

Every command ran from `ports/sfm/` inside a `ports/tools/build-slot.sh` lease
with `CARGO_INCREMENTAL=0`; no `--target-dir`, no bare `cargo` build.

## 1. The pure runtime (TDD)

| Step | Command | Result |
|---|---|---|
| red | `cargo test --offline` with the API stubs in place | `test result: FAILED. 0 passed; 27 failed` — every failure `not yet implemented: parse SFML` / `plan a program tick` (`../_shared/evidence/sfm/runtime-tdd-red.txt`) |
| green | `cargo test --offline` | `27 passed; 0 failed` |
| fmt | `cargo fmt --check` | clean |
| lint | `cargo clippy --all-targets --offline -- -D warnings` | clean |
| locked, final sources | `cargo fmt --check && cargo clippy --all-targets --locked --offline -- -D warnings && cargo test --locked --offline` | clean, `27 passed; 0 failed` (`../_shared/evidence/sfm/runtime-final.log`); `Cargo.lock` committed (13 packages) |

The 27 tests cover: `NAME`/`EVERY`/`SECONDS`/comments, quantity, `EACH`,
`RETAIN`, filters, `EXCEPT`, quoted and listed labels, `SIDE` lists, the three
parse errors this slice reports, the timer predicate, and twelve plans (whole
stack, shared quantity, per-id quantity, retention, filter, output quantity and
retention, `FORGET`, second output, off-network label, side choice, a
non-takable slot, an empty program).

## 2. The game ladder (pin `1d63e0a` / mixins `5a64af9`)

```sh
ports/tools/build-slot.sh run --job sfm-check --port ports/sfm --step check \
    -- env CARGO_INCREMENTAL=0 tools/ladder.sh check     # cargo_exit 0
ports/tools/build-slot.sh run --job sfm-test  --port ports/sfm --step test  \
    -- env CARGO_INCREMENTAL=0 tools/ladder.sh test      # 27 passed, cargo_exit 0
ports/tools/build-slot.sh run --job sfm-build --port ports/sfm --step build \
    -- env CARGO_INCREMENTAL=0 tools/ladder.sh build     # cargo_exit 0, 0 warnings
```

Logs: `../_shared/evidence/sfm/ladder-check-final.log`, `embedded-test-final.log`,
`horizon-build.log` (0 warnings in each).

`tools/ladder.sh` runs `stage` first: the runner deletes `stage/modded` before
each step and `tools/horizon.py` re-exports only when the stage signature
changes.

Embedded tests run as `game::sfm::tests` (`horizon.py test` passes
`--lib game::sfm::tests`), so the runtime is exercised inside the real game
crate as well as standalone.

## 3. Packaging

```sh
ports/tools/build-slot.sh run --job sfm-package --port ports/sfm --step package \
    -- env CARGO_INCREMENTAL=0 tools/package.sh
```

- `content/sfm.hmod` — 7,569 B, SHA-256
  `872e5fe6eb6656903e5d4941d54d0e2139381f2c7d626231273d7ba77b20d5c4`, 23 entries,
  `sfm 0.1.0 (mod, API 0.2, both)`, `no code: content only`.
- `sfm.hnmod` — 24,573 B, SHA-256
  `a90c4c5e95a0abffbe1c96d13b0e710fdf1dac40862a15141bb14639b3d0fbc9`, 10 entries
  (`horizon-native.json`, `mixins/sfm/{horizon-mixin.toml, runtime.rs,
  support.rs, block_entity.rs, level_tick.rs, register.rs, command.rs,
  embedded_content.rs, data/sfm-content.hmod}`), checked with
  `cargo horizon-mod check sfm.hnmod --kind native-mod`.
- A second `tools/package.sh` on the same sources reproduced both SHA-256s
  (`../_shared/evidence/sfm/package-repro.log`).
- `mixins/sfm/data/sfm-content.hmod` is byte-identical to `content/sfm.hmod`
  (the packager refuses anything else).
- Every `.rs`/`.toml` inside `sfm.hnmod` was compared against
  `mixins/sfm/` byte for byte: all identical, so the bundle is this source.

## 4. In game (`tools/e2e.sh`)

| Leg | Command | Result | Log |
|---|---|---|---|
| embedded (shipping path, no `--dev-mod`) + unusable-`.horizon-cache` control | `tools/e2e.sh` | `PASS` + `EXIT:0` | `../_shared/evidence/sfm/e2e-embedded-final.log` |
| content as a dev mod | `tools/e2e.sh dev` | `PASS` + `EXIT:0` | `e2e-dev-mod.log` |
| fail-capability | `tools/e2e.sh selftest` | `FAIL` + `EXIT:1` at the D assertion | `e2e-selftest.log` |

The default run also logs, before the scenario: the control leg (the same binary
with `.horizon-cache` a plain file — `sfm:manager` is unknown, the block stays
`minecraft:air`) and the embedded-content byte check (`sfm-embedded-content.hmod`
identical to `content/sfm.hmod`, 7,569 B).

```sh
python3 ports/tools/check-e2e-logs.py sfm --strict   # every log ends with a verdict
```

## 26.3 dialect sweep (2026-11-21) — 0.2.2

| Check | Command | Result |
|---|---|---|
| static dialect | `python3 ports/tools/check_261_dialect.py sfm.hmod` | exit 0; pre-fix package exit 1 (`../_shared/evidence/sfm/dialect-check-red.log`) |
| tests | `cargo test --offline` | exit 0 (`dialect-test.log`) |
| package | `cargo horizon-mod build` twice after `rm -f Cargo.lock` | `cmp`-identical, built against 0a875da |
| registry check | `cargo horizon-mod check sfm.hmod` | **48,954 B**, SHA-256 `a7ba227a4a9fe66f7f077168a87511ec8bfb363bebe9dd104437bdab21679f33`, exit 0 |
| fits the pin | `cargo horizon-mod fits sfm.hmod --build 0a875da` | `0a875da  made for`, exit 0 |
| the gate | `python3 ports/tools/verify-sdk-package.py sfm` | 1/1 |
| in game | `tools/e2e-dialect.sh` | `PASS` + `EXIT:0`; `SELFTEST=1` `FAIL` + `EXIT:1`; `check-e2e-logs.py sfm --strict` exit 0 |

Changed: 2 recipes + 2 loot tables. Proof: cable crafts 16 and drops 1; pre-fix red. Full record in [`notes.md`](notes.md) §"26.3 dialect sweep".
