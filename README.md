# Super Factory Manager ( SFM ) — a Horizon port

**Super Factory Manager ( SFM )** is a Minecraft mod. Adds a domain-specific language for logistics automation. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [TeamDman/SuperFactoryManager](https://github.com/TeamDman/SuperFactoryManager) — licence **MPL-2.0**.

| | |
|---|---|
| Upstream mod | Super Factory Manager ( SFM ) ([source](https://github.com/TeamDman/SuperFactoryManager)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `sfm.hmod` (install it with the Horizon launcher) |
| Licence | **MPL-2.0** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

### What works

- **The Factory Manager block** (`sfm:manager`) and the **inventory cable**
  (`sfm:cable`), carried from the jar's own models and textures.
- **An SFML program** runs on the manager's own tick:
  `EVERY <n> TICKS|SECONDS DO … END` with `INPUT`/`OUTPUT` statements —
  quantity, `EACH`, `RETAIN`, resource ids and `*globs*`, `EXCEPT`, labels
  (bare or quoted) and a `SIDE` list — plus `FORGET`.
- **Item movement over the cable network**: a label may sit on any block, but
  the manager only reaches blocks that are a cable of its network or touch one
  (upstream's rule). One trigger moves whole stacks; what a target refuses goes
  back into the source in the same transaction, so nothing is lost.

## Not yet ported

- **No in-game re-run in this environment** (trap 15). The 0.1.0 runs in
- **`Slot::can_take` is always `true` in the glue** (trap 7): a side's capability lists the slots it
- **A labelled block whose block entity is another mod's** is read through its `ITEMS` capability,
- **The manager's block entity type is now declared by the mod** (`sfm:factory_manager`), so the

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `sfm.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

Super Factory Manager ( SFM ) was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **MPL-2.0**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/TeamDman/SuperFactoryManager

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run sfm` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/sfm/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
