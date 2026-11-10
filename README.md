# sfm (Horizon port)

A Horizon port of [sfm](unknown) — MPL-2.0.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- In:** the Factory Manager and the plain inventory cable, the program text and
- the label table (upstream keeps both on the program disk), a compiled SFML subset,
- and the item movement one program tick makes over the manager's cable network.
- `NAME "…"`, `EVERY <n> TICKS|SECONDS DO … END` (`SECONDS` = 20 ticks, offset 0,
- `LOCAL` alignment), `INPUT`/`OUTPUT` with an optional quantity, `EACH`,
- `RETAIN n [EACH]`, a resource list (`stone`, `minecraft:redstone`, `*ingot*`),
- `EXCEPT …`, a label list (bare or quoted) and a `SIDE` list, and `FORGET`.
- Defaults as upstream: `INPUT` = take all / leave none, `OUTPUT` = accept all /

## Not yet ported

- Language: `IF`/`THEN`/`ELSE`, `REDSTONE TRIGGER`, `EMPTY SLOTS IN`, slot ranges,
- `ROUND ROBIN`, per-block `EACH`, quoted regex patterns, `MAX`/`EACH` on retention
- per id combination, `WITH` clauses, fluid/energy/chemical/redstone resources,
- `SIDE` forms `LEFT/RIGHT/FRONT/BACK` (they parse and name no face here).
- Content: disk, label gun, network tool, form/form_base, printing press, buffers,
- water tank, XP goop and shard, all fancy/tough/tunnelled cables and every facade,
- their recipes/loot/advancements, `c:` tag compatibility, the manager GUI,
- the printing press and its data recipes.

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: unknown. Port: AdLf0208. Licence: MPL-2.0 — see [LICENSE](LICENSE);
the original notices are preserved.
