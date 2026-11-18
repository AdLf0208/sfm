# sfm — Super Factory Manager 4.34.0 (Horizon port, slice 1) — 0.2.0 SDK package

**Format:** 0.2.0 is the flat SDK package (`horizon-mod.toml`, `src/lib.rs`, `src/support.rs`,
`src/runtime.rs`, `assets/`, `data/`); 0.1.0 was the retired `.hnmod`. See
[`## Migration to the SDK format`](#migration-to-the-sdk-format-020-2026-11-18) for the file map,
the hook table and the traps, and `../_shared/evidence/sfm/retired-hnmod-0.1.0/` for the retired tree.

**Class / tier:** campaign row says `content` / **`deep`** (`ports/ledger.csv`,
`ports/_shared/source-map/atm10-sources.csv`); this brief says `content` /
**`machines`**. The slice is machines-shaped — a block entity, item containers,
a tick and a language — and neither the engine nor the ledger row was re-tiered
here; the discrepancy is recorded for the orchestrator (§9).
**Licence:** **MPL-2.0**, read from the jar's
`META-INF/neoforge.mods.toml` (`license="Mozilla Public License Version 2.0"`);
the jar ships no LICENSE/NOTICE, so the upstream repository's `LICENSE.txt`
(MPL-2.0) is kept at `LICENSE` with `LICENSE-NOTICE.md` naming its source.
**Rank 0** in the campaign table (permissive family; still local-only until the
maintainer approves publishing). Jar evidence:
`ports/_shared/licence-cache/Super Factory Manager (SFM)-MC1.21.1-4.34.0.jar`.

## 1. The slice boundary

**In:** the Factory Manager and the plain inventory cable, the program text and
the label table (upstream keeps both on the program disk), a compiled SFML subset,
and the item movement one program tick makes over the manager's cable network.

- `NAME "…"`, `EVERY <n> TICKS|SECONDS DO … END` (`SECONDS` = 20 ticks, offset 0,
  `LOCAL` alignment), `INPUT`/`OUTPUT` with an optional quantity, `EACH`,
  `RETAIN n [EACH]`, a resource list (`stone`, `minecraft:redstone`, `*ingot*`),
  `EXCEPT …`, a label list (bare or quoted) and a `SIDE` list, and `FORGET`.
- Defaults as upstream: `INPUT` = take all / leave none, `OUTPUT` = accept all /
  no restraint.
- Movement: one whole stack quantum at a time, through
  `native_move_items` (never loses an item), with input retention per slot and
  output retention measured against what the target already holds.
- Reach: a label may sit anywhere, but the block must be a cable of the
  manager's network or touch one, as upstream's `isAdjacentToCable` insists.

**Out (gaps, §6):** the disk and label gun items, the manager text editor and
screen, the printing press, fancy/tough/tunnelled cables and facades, buffers,
the water tank, the XP items, fluids/energy/chemicals/redstone as resources,
`IF`/`REDSTONE TRIGGER`, `EMPTY SLOTS IN`, slot ranges, `ROUND ROBIN`, per-block
`EACH`, quoted regex patterns, `LEFT/RIGHT/FRONT/BACK` side resolution.

## 2. Upstream files read (behaviour, not code)

Repository `https://github.com/TeamDman/SuperFactoryManager`, tag
`4.34.0-1.21.1`, commit `f5366c79c823ff52712130e69dd9c8166c70bd14` (shallow
clone under `ports/_shared/upstream/sfm/`, deleted after `FINDINGS.md`):

- `ca/teamdman/sfm/common/blockentity/ManagerBlockEntity.java` — `serverTick`
  (`tick++` before the program tick, `random.nextInt()` on load, one disk slot),
  `State` (`NO_DISK`/`NO_PROGRAM`/`INVALID_PROGRAM`/`RUNNING`).
- `ca/teamdman/sfml/ast/{Program,TimerTrigger,Interval,InputStatement,
  OutputStatement,ResourceLimit,Limit,ResourceLimits,LabelAccess,Side}.java`.
- `ca/teamdman/sfml/ast/ASTBuilder.java` — the `SECONDS × 20`, default offset 0,
  `LOCAL` alignment, and the two default limits (`TAKE_ALL_LEAVE_NONE`,
  `ACCEPT_ALL_WITHOUT_RESTRAINT`).
- `ca/teamdman/sfm/common/program/{ProgramContext,SharedQuantitySharedRetention*,
  SharedQuantityExpandedRetention*,ExpandedQuantity*,LimitedInputSlot,
  LimitedOutputSlot}.java` — gather/retention/transfer accounting.
- `ca/teamdman/sfm/common/block_network/{CableNetwork,CableNetworkManager,
  ICableBlock}.java` and `ca/teamdman/sfm/common/capability/
  SFMBlockCapabilityDiscovery.java` — the network and the adjacent-cable rule.
- `ca/teamdman/sfm/common/item/{DiskItem,LabelGunItem}.java`,
  `ca/teamdman/sfm/common/label/LabelPositionHolder.java` — what the disk holds.
- `ca/teamdman/sfm/common/block/{ManagerBlock,CableBlock}.java`,
  `ca/teamdman/sfm/common/registry/registration/SFMBlocks.java` — block
  properties (manager `destroyTime(2)`, cable `destroyTime(1)`, both
  `SoundType.METAL`).
- `platform/minecraft/src/main/antlr/sfml/SFML.g4` — the grammar's line shape.
- The jar's `assets/sfm/template_programs/*.sfml` (18 programs: the language's
  own documentation) and `assets/sfm/blockstates|models|lang`, `data/sfm/*`.

Every number in `runtime.rs` names its source in a comment: `TICKS_PER_SECOND`
= 20 (`ASTBuilder` `ticks *= 20`), the timer predicate
`manager_tick % interval == 0` (`Interval.shouldTick`, `LOCAL`, offset 0), the
two default limits (`ASTBuilder:575–588`), block hardness/sounds (above).

## 3. How it runs

`runtime.rs` — pure: the parser, the timer predicate, the plan one trigger makes
(`plan(trigger, world) -> Vec<Move>`) and the trackers (shared/`EACH` quantity,
per-slot input retention, destination retention). No game types, 27 unit tests.

`support.rs` — glue: the manager's block entity (`Program` and `Labels` in its
saved compound), the tick at `level_tick_post` (the entity's own tick reverts
writes at this pin), the cable network walk, `native_container_slots` for every
labelled reachable container through each side, and `/sfm …`.

The tick: for each manager, `tick += 1`, compile (cached by a hash of the text),
for every trigger where `due(trigger, tick)`, build the snapshot, plan and run
the moves with `native_move_items(dimension, from, from_side, to, to_side,
Some(item), max)`.

`mixins/sfm/horizon-mixin.toml` — one module for the runtime, one for the glue,
and five `[[injection]]`s: `block_entity` (v1), `level_tick_post` (v1),
`register_commands` (v1), `perform_command` (v1), `content_root` (the AE2
embedded-content pattern).

## 4. Deviations (all deliberate)

1. **The program and the labels live on the manager, not on a disk.** Upstream
   keeps them in `sfm:disk`'s data components inside the manager's single slot;
   this slice has no disk item and no text editor, so the manager's own saved
   compound holds `Program` (string) and `Labels` (label → `"x, y, z"` list).
   The disk, label gun and screen are the deferred client half.
2. **`/sfm program <manager x y z> <program…>` replaces the editor GUI**, and
   `;` stands for a line break because a chat line cannot hold one (256 chars).
   `/sfm label <manager x y z> <block x y z> <name>` is the label gun's click;
   `/sfm unlabel`, `/sfm labels`, `/sfm show`, and `/sfm tick <manager> <n>`
   (harness control for the timer) complete it. Precedent:
   `/charginggadgets item`, `/ftbultimine shape`.
3. **The tick runs at `level_tick_post`, not in the block entity's own tick** —
   the pin reverts an entity's own writes there (`ARCHITECTURE.md` §3), and the
   manager must write nothing but its own program state. Same pattern as
   `moderndynamics`.
4. **The tick counter starts at 0 and is not saved**; upstream seeds it with
   `random.nextInt()` on load and saves nothing either, so the only difference
   is the phase of `EVERY n TICKS` after a chunk loads.
5. **Movement is per (container, face, item), not per slot.**
   `native_move_items` is the lossless helper (an item the target refuses stays),
   so `RETAIN` is honoured per container face for the item rather than per slot;
   for one stack per item per chest (the normal case) the two agree.
6. **Unqualified access uses each distinct set of slots once.** A chest reports
   the same 27 slots through every face (one move), a furnace reports input,
   fuel and output through different faces (three moves); upstream gathers every
   direction and relies on the empty-slot checks.
7. **A label set twice on one statement** gathers the slots twice, as upstream's
   `getLabelledPositions` does.
8. `EXCEPT` runs after the quantity/retention accounting has selected the slot,
   as upstream's trackers do (the exclusion list is a statement-level filter).
9. **`EACH` after `FROM`/`TO` (a tracker per block) is parsed and inert**; the
   `EACH` on a quantity/retention is implemented.
10. **Each due trigger is planned on its own.** Upstream ticks every due trigger
    of one program inside one `ProgramContext`, so an `INPUT` in one trigger can
    feed an `OUTPUT` in another; here each trigger gathers and moves its own
    items. A program whose triggers rely on each other (rare; every jar template
    keeps input and output in one trigger) behaves differently — recorded gap.

## 5. Verification (evidence)

| Step | Command (all in a `build-slot.sh` lease) | Result |
|---|---|---|
| runtime red | `cargo test --offline` on the stubs | 0 passed, 27 failed, `not yet implemented: parse SFML` / `plan a program tick` — `_shared/evidence/sfm/runtime-tdd-red.txt` |
| runtime | `cargo test --offline` | **27 passed, 0 failed** |
| fmt / lint / lock | `cargo fmt --check`, `cargo clippy --all-targets --locked --offline -- -D warnings`, `cargo test --locked --offline` | clean, **27 passed** — `_shared/evidence/sfm/runtime-final.log` (`Cargo.lock` committed, 13 packages) |
| stage+check | `tools/ladder.sh check` | `cargo_exit 0`, 0 warnings — `_shared/evidence/sfm/ladder-check-final.log` |
| embedded test | `tools/ladder.sh test` | **27 passed**, 1240 filtered out, `cargo_exit 0` — `embedded-test-final.log` |
| build | `tools/ladder.sh build`, and the e2e's own rebuild of the final sources | `cargo_exit 0`, **0 warnings** (`horizon-build.log`, `e2e-embedded-final.log`) |
| content | `tools/package.sh` | `sfm 0.1.0 (mod, API 0.2, both)`, `no code: content only` |
| reproducible | `tools/package.sh` a second time | the same two SHA-256s — `_shared/evidence/sfm/package-repro.log` |
| bundle check | `cargo horizon-mod check sfm.hnmod --kind native-mod` | `sfm 0.1.0 (native-mod, native-1/1d63e0a5…/5a64af93…)` |
| e2e (embedded, shipping path) | `tools/e2e.sh` (no `--dev-mod`, control leg) | **PASS + EXIT:0** — `_shared/evidence/sfm/e2e-embedded-final.log` |
| e2e (dev-mod) | `tools/e2e.sh dev` | **PASS + EXIT:0** — `e2e-dev-mod.log` |
| e2e selftest | `tools/e2e.sh selftest` | **FAIL + EXIT:1** at the D assertion — `e2e-selftest.log` |

What the e2e proves in game (The End, headless, creative):

- **A** a `setblock`-placed manager has a block entity: `/sfm show` reads
  `state=no_program triggers=0`; **B** three labels land on the three chests
  (`labels=3`, one block each); **C** the program compiles (`state=running
  triggers=1`);
- **D** five diamonds move a → b on the 20-tick timer (`a: 5 -> 0, b: 0 -> 5`);
  **D2** one firing moves exactly three more and the state line says
  `moved=3 moves=1`;
- **E** control: `OUTPUT TO c`, a label on a chest with no cable beside it,
  moves nothing (`a: 5, c: 0` — upstream's missing-adjacent-cable rule);
- **F** `EVERY 40 TICKS` moves nothing at 25 ticks and everything by 50 (the
  interval is the trigger's, not one per tick);
- **G** `INPUT 3 FROM a` moves exactly 3 of 5 and leaves 2;
- **H** control: `FORGET` between the input and the output moves nothing;
- **I** `OUTPUT TO EMPTY SLOTS IN b` is stored but reported
  `state=invalid` with `program error: line 2: EMPTY SLOTS IN is not in this
  slice (EMPTY follows TO)` — an unreadable program is inert, never a crash;
- **J** after a quit and relaunch the manager, its program and its labels come
  back (`state=running triggers=1 labels=3`) and it moves again;
- the **control leg** runs the same binary with `.horizon-cache` made a plain
  file: `sfm:manager` is unknown, so the ids really come from the embedded
  content, and the embedded file on disk is byte-identical to
  `content/sfm.hmod`.

Delivered (in the port, never `target/`):

- `content/sfm.hmod` — **7,569 B**, SHA-256
  `872e5fe6eb6656903e5d4941d54d0e2139381f2c7d626231273d7ba77b20d5c4`, 23 entries;
- `sfm.hnmod` — **24,573 B**, SHA-256
  `a90c4c5e95a0abffbe1c96d13b0e710fdf1dac40862a15141bb14639b3d0fbc9`,
  10 entries, carrying `mixins/sfm/data/sfm-content.hmod` (identical bytes);
- rebuilt twice from the same sources with the same SHA-256.

## 6. Gaps

Language: `IF`/`THEN`/`ELSE`, `REDSTONE TRIGGER`, `EMPTY SLOTS IN`, slot ranges,
`ROUND ROBIN`, per-block `EACH`, quoted regex patterns, `MAX`/`EACH` on retention
per id combination, `WITH` clauses, fluid/energy/chemical/redstone resources,
`SIDE` forms `LEFT/RIGHT/FRONT/BACK` (they parse and name no face here).
Content: disk, label gun, network tool, form/form_base, printing press, buffers,
water tank, XP goop and shard, all fancy/tough/tunnelled cables and every facade,
their recipes/loot/advancements, `c:` tag compatibility, the manager GUI,
the printing press and its data recipes.
Engine: the manager cannot be configured (upstream's config screen), programs
are limited to a chat line (256 characters) until a disk/editor exists, the
`RUNNING` state is not mirrored onto the block state, no sound/particles/animation,
and a chunk unloaded at a due tick simply misses that tick (upstream too).

## 7. Environment note (cost a run)

The first three vanilla builds failed with `could not parse/generate dep info
(os error 2)`: several jobs were sharing one build slot at the time (the
registry showed multiple `unrecorded` jobs), which is the shared-lease defect
`AGENTS.md` now documents. A plain retry in a clean lease built in 17m40s;
nothing in the port was wrong.

## 8. Build slots and the ladder

Every step ran through `../../ports/tools/build-slot.sh` from the port
directory, with `CARGO_INCREMENTAL=0`, never `--target-dir` and never a bare
`cargo` for a build/test. `tools/ladder.sh <stage|check|test|build>` exists
because the runner deletes `stage/modded` before each step, so `stage` has to
run in the same lease as the step that reads it.

## 9. For the orchestrator

- Ledger row: `sfm, Super Factory Manager (SFM), content, deep, MPL-2.0,
  ported-partial, P0, 4 hooks, ports/sfm`. Status `ported-partial` because the
  language and the client half (disk/label gun/GUI) are not ported. The supplied
  hooks are `block_entity`, `level_tick_post`, `register_commands`,
  `perform_command` and the content-root target (not a catalogue hook).
- Tier discrepancy: the ledger and the source map say `deep`, the brief says
  `machines`; the slice is machines-shaped and the row is unchanged.
- `upstream_ready.py sfm --copyleft` exits 1 **only** on the status gate while
  the row says `pending`; everything else it checks (artifact, `Skills:` line,
  e2e PASS + EXIT:0, the pin) is green. The flag is needed because the tool's
  reviewable list calls MPL-2.0 copyleft for submission policy, while this
  campaign's table ranks it **0 (permissive family)** — `precheck-mod.py sfm`
  prints `licence=MPL-2.0 rank=0 (permissive)`. Both are recorded rather than
  reconciled here; publishing still needs the maintainer (§ARCHITECTURE 8).

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Review repairs (PR #640, 2026-11-11)

Review: `ports/_shared/horizon-mods/reviews/pr-640-sfm.md` (CHANGES_REQUESTED, 2 items).

| Reviewer's item | This job |
|---|---|
| 1. artifact not in the registry release/staging path | **orchestrator** (no write access) |
| 2. `[sfm](unknown)`, only the porter credited, truncated description (`OUTPUT = accept all /`, broken `In:**`) | link and authors were already `https://github.com/TeamDman/SuperFactoryManager` / TeamDman; the **description is rewritten** with the compiled SFML subset, upstream's INPUT/OUTPUT defaults, and the language/content gap list |

The reviewer could not read MPL source under their rules; the port carries the MPL-2.0
text and the original copyright notice in the archive (`mixins/sfm/LICENSE`,
`LICENSE-NOTICE.md`) and its complete corresponding source is in the repository, so a
permitted reviewer has everything needed.

Artifact: `sfm.hnmod` **37,934 B** `e57f9ccd77498c88276f10f1010b2c708ff6b15f7b74d79e5551035fad80b7d8`
(content `sfm.hmod` **17,164 B** `1716d15d18b881c545d55fd2c4b0b6cf40e3f6b262eed6c2fd394477bffd20e7`).

## Migration to the SDK format (0.2.0, 2026-11-18)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70, PR #17992) retired `.hnmod`: the registry closed
every `.hnmod` submission on 2026-11-18, and an `.hnmod` no longer loads. This port's former
submission, **PR #807** (`sfm` 0.1.0), was closed by that purge. **0.2.0** is the same behaviour
republished as one flat SDK `.hmod`; the changelog text is at the end of this section.

Pins: SDK nightly **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** (2026-11-18, `package_format` 6,
`content_api` "0.2", toolchain rust 1.99.1) and mixins **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**
(the mixins commit moved: the 0.1.0 campaign pin was `5a64af93`). The SDK is cached in-tree at
`../_shared/sdk/0b14f8a/` and was copied into `.horizon/sdk/` (gitignored) for offline builds.

**Done in this order:** the retired layout was moved into
`../_shared/evidence/sfm/retired-hnmod-0.1.0/` **before** the first `cargo horizon-mod build` (the
tool refuses a root holding retired package paths); `src/runtime.rs` was copied with `cp` and proved
with `cmp` + `sha256sum`; `cargo test` ran on the new crate **before any glue existed** (**27
passed**); then the glue was written test-first (6 `support::` tests red on `todo!()`, then green —
**33 passed**).

### The licence (read, not assumed)

`python3 ports/tools/precheck-mod.py sfm` prints **`licence=MPL-2.0 rank=0 (permissive)`**, read from
`../_shared/licence-cache/Super Factory Manager (SFM)-MC1.21.1-4.34.0.jar`
(`META-INF/neoforge.mods.toml`, `license="Mozilla Public License Version 2.0"`). The brief said "none
recorded"; the artefact and the tool agree it is recorded, so the licence is **MPL-2.0, rank 0** and
the port stays **local-only until the maintainer approves publishing** (ARCHITECTURE §8). The port's
`LICENSE` (MPL-2.0 text) and `LICENSE-NOTICE.md` are packed at the package root.

### What moved, and where the retired tree is

| 0.1.0 path | Now | Evidence |
|---|---|---|
| `mixins/sfm/runtime.rs` (1,170 lines, 27 tests) | `src/runtime.rs`, **copied with `cp`, `cmp`-checked** | `retired-hnmod-0.1.0/mixins/sfm/runtime.rs` |
| `mixins/sfm/support.rs` (615 lines) + 5 hook helpers (`block_entity.rs`, `level_tick.rs`, `register.rs`, `command.rs`, `embedded_content.rs`) | `src/support.rs` (game glue) + `src/lib.rs` (the one `Hooks`, the Part) | `retired-hnmod-0.1.0/mixins/sfm/` |
| `mixins/sfm/horizon-mixin.toml` (2 `[[module]]`, 5 `[[injection]]`) | no plan: one `Registrar` declaration, typed `Hooks`/Part methods | `retired-hnmod-0.1.0/mixins/sfm/` |
| `mixins/sfm/data/sfm-content.hmod` + `embedded_content.rs` (AE2 write-out) | **gone**: code and content are one package, so there is nothing to embed | `retired-hnmod-0.1.0/mixins/sfm/` |
| `content/` (`horizon-mod.toml`, `assets/`, `data/`, the built `sfm.hmod`, `target/`) | the package root: `assets/`, `data/` (all 22 files byte-identical), the root `horizon-mod.toml` | `retired-hnmod-0.1.0/content/` |
| `sfm.hnmod` (37,934 B) | `sfm.hmod` (45,937 B, native + content) | `retired-hnmod-0.1.0/sfm.hnmod` |
| `compatibility.json`, `rust-toolchain.toml`, `Cargo.lock` | `[native]` in `horizon-mod.toml`; the pinned commit decides the toolchain; no lock is packed | `retired-hnmod-0.1.0/` |
| `tools/horizon.py`, `tools/package.py`, `tools/package.sh`, `tools/test_stage.py`, `tools/test_template.py`, `tools/audit/` | deleted: staging, `.hnmod` packaging and the plugin-injection audit belong to the retired format | `retired-hnmod-0.1.0/tools/` |
| **also** `tools/ladder.sh`, `tools/notice.py` | deleted: `ladder.sh` drives the deleted `tools/horizon.py`, and `notice.py` injects the licence into a content-only `.hmod` (the root `LICENSE` is packed now). The brief did not list them; the artefact did — a tool that calls a deleted tool is retired tooling | `retired-hnmod-0.1.0/tools/` |

**Kept:** `tools/e2e.sh` (scenario and assertions format-independent; the launch line and the
embedded-content legs changed — see trap 15), `notes.md`, `README.md`, `FINDINGS.md`, `VALIDATION.md`,
`LICENSE`, `LICENSE-NOTICE.md`.

### File map (0.2.0)

| Path | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest and identity: `id = "sfm"`, `version = "0.2.0"`, `api = "0.2"`, `side = "both"`, `capabilities = ["registries"]`, `[native] game = "26.3"`, `horizon = 0b14f8a…`, `mixins = e59da90…` |
| `Cargo.toml` | one crate, `edition = "2024"`; its only dependency is `horizon-mod-api` from `.horizon/sdk/`; `[lib] path = "src/lib.rs"` |
| `src/lib.rs` (2,386 B) | the one `#[mod_api::horizon_mod] impl Hooks`: `register`, `register_commands`, `run_mod_command`, `level_tick_post` |
| `src/support.rs` (32,789 B) | the glue: the manager's block entity Part, the tick queue, the network walk, the container reads, the moves, `/sfm`, and the saved-compound codec. 6 unit tests |
| `src/runtime.rs` (40,414 B) | the pure truth (parser, timer, plan, retention) — **byte-identical**, 27 tests |
| `assets/sfm/`, `data/` | the carried content, moved from `content/` (11 asset files, 11 data files) |
| `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, `icon.png` | packed at the root, so a downloaded copy carries them |
| `tools/e2e.sh` | the in-game scenario, updated for a launcher-built game |
| `sfm.hmod` | the deliverable: 45,937 B, sha256 `12d4f61c67a9d01420f5f29f1180de949308c4116623e6a56e2c745ac8f33348`, 32 entries |

### Helper → hook table

| 0.1.0 injection / helper | 0.2.0 place | Mapping notes |
|---|---|---|
| `embedded-content` (`content_root` HEAD, cancellable) | **none** | a flat `.hmod` carries `assets/`+`data/`; the write-out helper and `native_data!` are gone |
| `block-entity` (`block_entity` v1, returned `Some((MANAGER, Compound::new()))`) | `Registrar::block_entity_type("factory_manager").blocks(&["sfm:manager"]).build()` + `Registrar::block_entity_part`, in `Hooks::register` | the declaration owns the type id and creates the entity; a fresh one starts with an **empty** compound — exactly the 0.1.0 initial state, so no `block_entity` Part method is needed |
| `level-tick-post` (`level_tick_post` v1) + the enumeration of `game.world.block_entities` | `BlockEntityPart::block_entity_tick` **queues** `(level, pos)`; `Hooks::level_tick_post` drains the queue and runs the programs | the SDK cannot enumerate a level's block entities, and a block entity is out of the world during its own tick (trap 3) |
| `register-commands` (`register_commands` v1) | `Hooks::register_commands` → `support::declare_commands` | the same six usages and levels; `NativeCommand::new`/`::op` are `Command::new`/`Command::op` |
| `perform-command` (`perform_command` v1, raw words) | `Hooks::run_mod_command` → `support::run_command` | typed `CommandInvocation`: `pos`/`target` are `BlockPos([i32;3])`, `program`/`name` are `String`, `value` is `Integer`. **The readout text is byte-identical** so the 0.1.0 e2e assertions still match (`SFM pos=… tick=… state=… triggers=… labels=… fired=… moved=… moves=…`, `program error: line 2: …`, `label "a": 1 block(s)`) |
| `[[module]] runtime` | `pub mod runtime;` | the file itself is byte-identical |
| `[[module]] state` (`sfm_state`) | `src/support.rs`, a normal module | no injected module, no `crate::game` path |

Game access, old → new:

| 0.1.0 | 0.2.0 |
|---|---|
| `game.world.block_entities` (enumeration and `&mut ModBlockEntity`) | `host.block_entity(level, pos)` → `BlockEntityRef::data` / `replace_data`; the tick queue replaces the enumeration |
| `game.native_block(pos)` (`String`) | `level.block(host, pos)` → `BlockKey`, compared with the registered `BlockKey` |
| `game.native_container_slots(dimension, pos, side)` | `level.capability(host, &ITEMS, pos, Some(direction))` → `ResourceHandler::{size, resource, amount}` |
| `game.native_move_items(…, Some(item), max)` | `extract` + `insert_any` inside `host.transaction`, the refused part put back into the source slot |
| `game.world.modified.insert(chunk)` | `BlockEntityRef::replace_data` (loads, `setChanged`, client update) |
| `game.native_feedback(Text::literal(…))` | `host.commands().feedback(Text::literal(…))` |
| `player.dimension` | `invocation.dimension.id(host)` → `host.level(&id)` |
| `horizon_world::nbt::{Compound, JavaString, Tag}` | `mod_api::{Compound, Tag}`; keys built with `"key".into()`, read through a UTF-16 comparison |

### The ladder (measured)

Every command ran in one `ports/tools/build-slot.sh` lease with `BUILD_LOCAL=1` (the runner forwards
to the remote box, where the cwd is the synced port — trap 11), `CARGO_INCREMENTAL=0`, no
`--target-dir`:

| Step | Command | Result |
|---|---|---|
| runtime only, before any glue | `cargo test --offline` | **27 passed**, 0 failed (`sdk-test-runtime-before-glue.log`) |
| glue, red | `cargo test --offline` | 27 passed, **6 failed on `todo!()`** (`sdk-test-red.log`) |
| tests | `cargo test --offline` | **33 passed** (27 `runtime::`, 6 `support::`), 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, 0 warnings (`sdk-clippy.log`) |
| format | `cargo fmt --check` | empty diff (`sdk-fmt.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` | `SDK 0b14f8a … in ./.horizon/sdk`, `Built ./target/horizon-mod/sfm.hmod` (`sdk-build.log`) |
| again | the same command | **45,937 B**, sha256 `12d4f61c…f33348` both times (`sdk-build-second.log`, `sdk-reproducible.txt`) |
| registry check | `cargo horizon-mod check sfm.hmod` | `sfm 0.2.0 (native-mod, format native-1/0b14f8acfd97fa2c750e42795249dcc981178ca4/e59da9094f0c2fdda6c627fe9eff22dec6a906f0, for 26.3)` / `45937 bytes, SHA-256 12d4f61c…, unsigned`, **exit 0** (`sdk-check.log`) |
| fits | `cargo horizon-mod fits sfm.hmod --build nightly` | `Super Factory Manager:` / `0b14f8a  made for`, **exit 0** (`sdk-fits.log`) |
| runtime untouched | `unzip -p sfm.hmod src/runtime.rs \| sha256sum` | `a4e8776c…a2cd8` = the retired file = `src/runtime.rs` (`sdk-runtime-sha256.txt`) |
| content carry | `sha256sum` of every carried file, before and after the move | 22/22 identical (`sdk-content-carry-before.txt` vs `-after.txt`) |

The archive's 32 entries: `horizon-mod.toml`, `Cargo.toml`, `LICENSE`, `LICENSE-NOTICE.md`,
`README.md`, `icon.png`, `src/lib.rs`, `src/runtime.rs`, `src/support.rs`, `uses.json`, 11
`assets/sfm/…`, 11 `data/…`. **No `Cargo.lock`.**

`uses.json` is the migration's summary (`sdk-uses-from-package.json`, `sdk-uses-summary.txt`): **275
surface entries** — 6 hooks (`block_entity`, `block_entity_ticks`, `block_entity_tick`,
`level_tick_post`, `register_commands`, `run_mod_command`) plus their field entries, `Hooks::register`
with `BlockEntityTypeBuilder`, 4 Part kinds, the `ResourceHandler` verbs (`size`, `resource`,
`amount`, `extract`, `insert`, `insert_any`), `HostTransaction::transaction`, `Commands::feedback` /
`run_command`, `BlockEntitiesHost::block_entity`, `Levels::list_levels`, both macros, and the
`horizon:items` and `registries` capabilities.

### Traps this migration hit (the next port must know)

1. **`LevelTickPostEvent` has no lifetime parameter.** `&mut LevelTickPostEvent<'_>` is
   `E0107: struct takes 0 lifetime arguments`; the other hook events (`BlockEntityTickEvent<'_>`,
   `RunModCommandEvent<'_>`) do take one. Read the generated struct, not the pattern of the last port.
2. **`level_tick_post` receives `HostSide`, not `&mut dyn ServerHost`.** A `side = "both"` hook gets
   `HostSide::Server(..)`/`Client(..)`; match it and let the client's level go.
3. **The SDK cannot enumerate a level's block entities.** 0.1.0 walked `game.world.block_entities` at
   `level_tick_post`; there is no replacement. A block entity ticks itself, so the manager **queues
   `(level, pos)` in `block_entity_tick`** and the pass drains that queue at `level_tick_post` — which
   is also where the network walk must happen, because a block entity is out of the world during its
   own tick.
4. **`merge_data` is a recursive merge and cannot delete a key.** Writing the label table with
   `merge_data` would leave a removed label in place (`/data merge block` never removes). The manager's
   payload is wholly its own, so label writes read it, edit it and `replace_data` the whole compound.
5. **`Compound`'s key type (`JavaString`) is not exported.** The label table is read into owned
   `(String, Vec<String>)` pairs and rebuilt whole with `"label".into()`; no key type is ever named
   (0.1.0's `JavaString::from(label)` does not compile here).
6. **`native_move_items` is gone.** A move is `ResourceHandler::extract` then `insert_any`; what the
   target refuses is put back into the source slot *inside the same `host.transaction`*, so a move
   still never loses an item. One transaction per program tick, as `laserio` does per network.
7. **`native_container_slots` is gone.** The side-filtered `ITEMS` capability lists exactly the slots a
   side reaches, so `runtime::Slot::can_take` is always `true` from the glue; the engine's `extract`
   is the last word. The runtime's own `a_slot_the_side_cannot_take_is_left_alone` test still covers
   the false case.
8. **The state key cannot be a dimension string.** Key on `(LevelRef, Pos)`: `LevelRef` is
   `Copy + Eq + Hash` but **not `Ord`**, so the map is a `HashMap` (nothing iterates it for output, so
   output order is unaffected).
9. **A declared block entity starts with an empty compound** — which is exactly what 0.1.0's
   `block_entity` helper returned, so the initial-state hook is dropped, not lost. Unlike
   `naturesaura`, no sentinel is needed here: an absent `Program` reads as "no program".
10. **`cargo fmt --check` formats the crate's module tree.** `rustfmt src/lib.rs` follows `mod
    runtime;` and would rewrite `src/runtime.rs`. Only `src/lib.rs` (with
    `--config skip_children=true`) and `src/support.rs` were formatted; `runtime.rs` stayed
    byte-identical (`cmp` + the packaged sha256).
11. **`build-slot.sh run` forwards to the remote runner, where the cwd is the synced port**, so the
    brief's `cd ports/sfm` fails with `No such file or directory`. Every step ran with
    `BUILD_LOCAL=1` (the local box had free slots), which keeps the brief's command shape.
12. **The root is packed.** `README.md`, `LICENSE`, `LICENSE-NOTICE.md` and `icon.png` travel inside
    the archive; the README was rewritten before the first build (the 0.1.0 text told the reader to
    run the deleted ladder and to expect an `.hnmod`).
13. **Two more retired tools than the brief listed.** `tools/ladder.sh` (calls `tools/horizon.py`) and
    `tools/notice.py` (injects notices into a content-only `.hmod`), plus `content/target/`, were also
    moved to the evidence folder: they belong to the retired format and would mislead the next reader.
14. **`/sfm tick` keys on the command's dimension, not the player's.** `invocation.dimension` is the
    documented route and equals the player's dimension for a chat command (the 0.1.0 value); `execute
    in` now moves the counter with it, which is the faithful reading of upstream's per-level manager.
15. **The 0.1.0 e2e cannot run the new package.** The script drove a `tools/horizon.py build` game with
    an embedded `.hmod`; a 0.2.0 `.hmod` is compiled in by
    `horizon-launcher native-build ports/sfm --instance <i> --trust-native-code`, which is not
    installed here. `tools/e2e.sh` now takes `GAME=` (a launcher-built game) and an optional
    `CONTROL_GAME=` (a game built **without** the package, where the ids must be unknown), keeps
    `SELFTEST=1`, and fails with `missing binary` rather than running an unmodded game.

### Behaviour that changed with the format (deliberate, and why)

- **The tick's transport, not its timing.** The pass still runs after the level's tick; it is now
  driven by each manager's own block-entity tick queueing itself instead of a global enumeration. A
  manager whose chunk is unloaded still misses that tick, and its counter is still dropped once it is
  gone (upstream too).
- **No log line** for anything: the SDK exports no logger and the package adds no crate. 0.1.0's one
  `log::warn!` (embedded content could not be written) no longer exists, because there is no
  embedded content to write.
- **The label table is rewritten whole** (`replace_data`) instead of a top-level key insert; the saved
  shape is unchanged (`Labels: { "a": ["4, 100, 0"] }`, `Program: "…"`), so a world saved by 0.1.0
  loads here and vice versa.
- **A move's "moved" count** is what the destination accepted, the same number
  `native_move_items` returned; the refused part returns to the source slot in the same transaction.

### Gaps after the migration

Everything in §6 (the language, the content, the engine limits) still stands. Added by this format:

1. **No in-game re-run in this environment** (trap 15). The 0.1.0 runs in
   `../_shared/evidence/sfm/` remain the behaviour's evidence; `src/runtime.rs` is byte-identical and
   the glue moved the same calls to the documented host API, which is what an in-game run would
   confirm.
2. **`Slot::can_take` is always `true` in the glue** (trap 7): a side's capability lists the slots it
   reaches, and the engine refuses an extract it does not allow. Observable only as a planned move
   that moves 0.
3. **A labelled block whose block entity is another mod's** is read through its `ITEMS` capability,
   which is the same route every automation mod uses; 0.1.0's raw container scan is gone. A block that
   answers no item capability is invisible, as it was.
4. **The manager's block entity type is now declared by the mod** (`sfm:factory_manager`), so the
   content's block JSON carries no block-entity declaration and a world created by 0.1.0 keeps its
   manager's saved data (the keys are unchanged).

## Upstream defects fixed

None found in this migration. The SFML subset, the timer predicate, the two default limits and the
adjacent-cable rule are the same behaviour 0.1.0 ported and tested; no artefact contradiction
(code against data, an unusable recipe, an off-by-one) was observed while re-reading the helpers. The
0.1.0 slice's own deviations (§4) stand as deviations, not defects.

### Changelog (0.2.0) — supersedes #807

> **0.2.0 — republished as an SDK `.hmod`.** Horizon's mod-API merge (launcher ≥ 0.2.70) retired
> `.hnmod` and closed **#807** (`sfm` 0.1.0); this version supersedes it. One flat `sfm.hmod` now
> carries the code *and* the content (the Factory Manager, the inventory cable, their models,
> textures, recipes, loot tables, lang and creative tab): `src/lib.rs` is the one
> `impl mod_api::Hooks`, `src/support.rs` the game glue, and `src/runtime.rs` is byte-identical to
> 0.1.0, so the parser, the timer, the plan and the retention rules behave exactly as before. Built
> against SDK `0b14f8a` / mixins `e59da90` for 26.3. What changed for `api = "0.2"`: the manager's
> block entity is a declared type with a Part instead of a `block_entity` injection, its tick queues
> the manager and the program's moves run at `level_tick_post` (the SDK cannot enumerate a level's
> block entities), item moves go through the `ITEMS` capability with the refused part returned in the
> same transaction instead of `native_move_items`, the label table is written with `replace_data`
> (a recursive merge cannot delete a label), `/sfm` is a typed `run_mod_command`, and the embedded
> content write-out (the AE2 pattern, the `.hnmod`'s only way to ship its blocks) is gone because the
> package carries them. Same feature set and the same gaps (the rest of SFML, the disk, label gun and
> GUI, the fancy cables and facades, fluids/energy/chemicals/redstone).

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port
