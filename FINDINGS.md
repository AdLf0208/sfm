# FINDINGS — Super Factory Manager (SFM) 4.34.0

Read from the pack's jar `Super Factory Manager (SFM)-MC1.21.1-4.34.0.jar` (kept at
`ports/_shared/licence-cache/`) and from the upstream repository
<https://github.com/TeamDman/SuperFactoryManager> at tag `4.34.0-1.21.1`
(commit `f5366c79c823ff52712130e69dd9c8166c70bd14`, shallow clone under
`ports/_shared/upstream/sfm/`, deleted after this file was written). Behaviour,
not code: the rewrite is written against the pinned Horizon hooks.

## Licence

`META-INF/neoforge.mods.toml`: `license="Mozilla Public License Version 2.0"`,
`modId="sfm"`, `version="4.34.0"`, authors `TeamDman`. The jar ships **no**
`LICENSE`/`NOTICE` file; the upstream repository's `LICENSE.txt` is the MPL-2.0
text and is kept at `ports/sfm/LICENSE` with `LICENSE-NOTICE.md` naming where it
was read. Rank 0 in the campaign's table (permissive family, still local-only
until the maintainer approves publishing).

## The jar's inventory (1,037 entries)

Blocks (`assets/sfm/blockstates/`): `manager`, `tunnelled_manager`, `cable`,
`fancy_cable`, `tough_cable`, `tough_fancy_cable`, `tunnelled_cable`,
`tunnelled_fancy_cable`, each with a `*_facade` variant for
`cable`/`fancy_cable`/`tough_*`/`tunnelled_*`, plus `buffer` (6 resource states),
`printing_press`, `water_tank`, `test_barrel`, `test_barrel_tank`.

Items (`assets/sfm/models/item/`): the block items above plus `disk`,
`labelgun`, `network_tool`, `form`, `form_base`, `xp_goop`, `xp_shard`.

Data: 17 recipes, 17 loot tables, 13 advancements, 2 block tags
(`minecraft:mineable/{axe,pickaxe}` — the manager needs a pickaxe); the recipes
use NeoForge `#c:` tags (`c:chests`, `c:dyes/*`) which 26.3 does not ship.

Resources the mod moves: items, fluids, energy (Forge/Mekanism), chemicals and
redstone, each a `ResourceType` with its own capability.

Other assets worth knowing: `assets/sfm/template_programs/*.sfml` (18 example
programs shipped in the jar, the language's own documentation),
`assets/sfm/textures/gui/container/manager.png` (the manager screen).

## The program language (SFML), as the example programs show it

From `assets/sfm/template_programs/`:

```
NAME "A simple program"

EVERY 20 TICKS DO
    -- comments are two dashes
    INPUT FROM a          -- an input on its own does nothing: there is no buffer
    OUTPUT TO b           -- the output moves what the inputs gathered
END
```

The families the jar's templates use: `NAME`, `EVERY n TICKS|SECONDS DO … END`,
`INPUT`/`OUTPUT` with `[quantity] [EACH] [RETAIN n [EACH]] [resource…]
[EXCEPT resource…] FROM|TO [EACH] label[, label…] [SIDE list]`, label names
bare or quoted (quoted allows spaces, e.g. `"to be smelted"`), `FORGET`,
resource patterns (`stone`, `minecraft:redstone`, globs `*ingot*`, quoted regex),
`EMPTY SLOTS IN`, `IF`/`REDSTONE TRIGGER`, `EACH` per block, slot ranges and
`ROUND ROBIN`.

## The manager's tick (`ManagerBlockEntity.serverTick`)

- A block entity with **one slot** (`ITEMS` = 1 slot) holding the program disk;
  the disk carries the program text and the label table
  (`LabelPositionHolder`: label → set of block positions) as data components.
- Every server tick: `manager.tick++`, then `program.tick(manager)`; the counter
  is `random.nextInt()` when the entity loads, so managers stagger.
- A trigger's timer is `Interval.shouldTick`: `manager.tick % ticks == offset`
  with `LOCAL` alignment and offset 0 by default (`GLOBAL` alignment uses the
  level's game time). The manager counts up **before** it ticks the program, so
  the first firing of `EVERY 20 TICKS` is at manager tick 20.
- `EVERY n SECONDS` is `20n` ticks (`TICKS_PER_SECOND` = 20).
- State shown to the player: `NO_DISK`, `NO_PROGRAM`, `INVALID_PROGRAM`,
  `RUNNING`.

## The network (`CableNetwork`, `SFMBlockCapabilityDiscovery`)

- A label may be put on **any** block, but the manager only reaches a block
  through its cable network: `discoverCapabilityFromNetwork` refuses a position
  that is not a cable of the network or adjacent to one, and logs
  `LOGS_MISSING_ADJACENT_CABLE`. `ManagerBlock` itself implements `ICableBlock`,
  so the manager is a member and its six neighbours are reachable without any
  cable.
- Cables are discovered as the contiguous run of `ICableBlock`s from the
  manager (6-neighbour flood).

## Item movement (`InputStatement`, `OutputStatement`, the trackers)

- `INPUT` gathers the slots of its labels into `LimitedInputSlot`s (one per
  label, position, side, slot); `OUTPUT` walks those slots into its own output
  slots.
- Defaults: `INPUT` = `TAKE_ALL_LEAVE_NONE` (quantity unlimited, retention 0,
  every resource), `OUTPUT` = `ACCEPT_ALL_WITHOUT_RESTRAINT` (quantity
  unlimited, retention unlimited).
- `quantity` (`Shared`) is one total across every gathered slot; `EACH` on the
  quantity/retention expands it per matching resource id.
- `retention` on `INPUT` dedicates up to n of **each slot** to what stays, with
  the total dedication capped by the statement's retention; on `OUTPUT` it is a
  total for the destination (`updateRetentionObservation` adds what the target
  already holds, so `OUTPUT RETAIN 8 …` tops the target up to 8).
- A move is one stack quantum at a time through the capability; an item the
  target refuses stays where it was.

## Sides (`Side`)

`TOP`, `BOTTOM`, `NORTH`, `SOUTH`, `EAST`, `WEST` map to the game's
`Direction`; `LEFT`, `RIGHT`, `FRONT`, `BACK` resolve through the block state's
`facing` (and fail where the block has none). A label access without `SIDE`
reaches the position through every direction.

## What this port takes (slice 1) and what it leaves

Taken: `NAME`, `EVERY n TICKS|SECONDS`, `INPUT`/`OUTPUT` with
quantity/`EACH`/`RETAIN`/resource lists/`EXCEPT`/labels/sides, `FORGET`, the
manager block, the plain cable and the network adjacency rule, the item moves on
the manager's own tick.

Left (recorded in `notes.md` as gaps): the disk and label gun items (the program
and labels live on the manager here), the manager GUI text editor, the printing
press, fancy/tough/tunnelled cables and facades, buffers, water tank, XP items,
fluids/energy/chemicals/redstone resources, `IF`/`REDSTONE TRIGGER`, `EMPTY SLOTS
IN`, slot ranges, `ROUND ROBIN`, per-block `EACH`, quoted regex patterns, and
`LEFT`/`RIGHT`/`FRONT`/`BACK` side resolution.
