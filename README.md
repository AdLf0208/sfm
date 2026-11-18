# Super Factory Manager for Horizon — slice 1 (0.2.0)

A clean-room Rust rewrite of [Super Factory Manager](https://github.com/TeamDman/SuperFactoryManager)
4.34.0 for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft 26.3).
Not affiliated with TeamDman; the original mod's MPL-2.0 licence and copyright
travel with this port (`LICENSE`, `LICENSE-NOTICE.md`).

One package: `sfm.hmod` carries the code (`src/`) and the content (`assets/`,
`data/` — the Factory Manager, the inventory cable, their models, textures,
recipes, loot tables, lang and creative tab).

## What works

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

## What it does not do yet

The program disk, the label gun, the manager's text editor and screen, the
printing press, the fancy/tough/tunnelled cables and facades, buffers, the water
tank, XP items, and every resource that is not an item (fluids, energy,
chemicals, redstone triggers). `IF`, `EMPTY SLOTS IN`, slot ranges and
`ROUND ROBIN` are not implemented; a program that uses them is stored but the
manager reports it as invalid instead of running it. The full list is in
`notes.md`.

## Using it

1. Place a **Factory Manager** and an **Inventory Cable** beside it; put a chest
   against the cable.
2. Put labels on the chests:
   `/sfm label <manager x y z> <chest x y z> a` (and `b` on the second chest).
3. Give the manager a program — `;` is a line break, because a chat line cannot
   hold one:
   `/sfm program <manager x y z> EVERY 20 TICKS DO;INPUT FROM a;OUTPUT TO b;END`
4. `/sfm show <manager x y z>` reads its state (`no_program`, `running`,
   `invalid`), its tick, its labels and what the last program tick moved.

Crafting: 16 cables from iron bars, a chest, black dye and a light weighted
pressure plate; a manager from chests, cables and a repeater.

## Building

The package is built by `cargo horizon-mod`, which fetches the pinned SDK into
`.horizon/sdk/` on first use:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
rm -f Cargo.lock && cargo horizon-mod build      # -> target/horizon-mod/sfm.hmod
cargo horizon-mod check sfm.hmod
cargo horizon-mod fits sfm.hmod --build nightly
```

Everything above runs inside a campaign build slot:
`ports/tools/build-slot.sh run --job sfm-sdk --port ports/sfm --step build -- …`.

`tools/e2e.sh` needs a game the launcher compiled this package into
(`horizon-launcher native-build ports/sfm --instance <instance> --trust-native-code`),
passed as `GAME=`; `SELFTEST=1` inverts one assertion so the run must fail.
No `horizon-launcher` is installed in this environment, so no 0.2.0 in-game run
is recorded (see `notes.md`).
