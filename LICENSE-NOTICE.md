# Licence notice — sfm (Super Factory Manager for Horizon)

## The original mod

- **Name:** Super Factory Manager (SFM), version 4.34.0 for Minecraft 1.21.1
  (NeoForge)
- **Author:** TeamDman — <https://github.com/TeamDman/SuperFactoryManager>
- **Licence:** Mozilla Public License Version 2.0 (MPL-2.0), as declared in the
  jar's `META-INF/neoforge.mods.toml` (`license="Mozilla Public License Version
  2.0"`). The jar carries no LICENSE file of its own, so the licence text kept
  beside this notice is the upstream repository's `LICENSE.txt` at tag
  `4.34.0-1.21.1` (commit `f5366c79c823ff52712130e69dd9c8166c70bd14`), which is
  the same MPL-2.0 text; see `LICENSE`.
- **What was copied:** the block and item models, blockstates, textures, lang
  keys, loot tables and recipe *shapes*, and the mod's own name for its blocks
  and creative tab. They are the original mod's assets, carried to 26.3 and
  kept under the MPL-2.0.
- **What was rewritten:** all behaviour. The Java source was read for behaviour
  only (which statements exist, how the timer, the network, the retentions and
  the defaults work); the Rust in `mixins/sfm/` is a clean-room rewrite against
  the pinned Horizon hooks, not a translation of decompiled code.

## This port

- **Licence:** MPL-2.0, the same as the original. Redistribution and
  modification are permitted under its terms, provided the complete
  corresponding source travels with the binaries and the notices are kept.
- **Source:** `mixins/sfm/*.rs` and `mixins/sfm/horizon-mixin.toml` (the bundle),
  `runtime.rs` (the pure half, also this crate's library), `content/` (the
  content mod's sources) — everything needed to rebuild `sfm.hnmod` and
  `content/sfm.hmod`.
- **Status:** local-only until the campaign's maintainer approves publishing.
  Nothing here has been published.

## Provenance of the pack jar

`Super Factory Manager (SFM)-MC1.21.1-4.34.0.jar`, kept as evidence at
`ports/_shared/licence-cache/`, matched the ATM10 pack's record for the mod. The
port's `compatibility.json` pins Horizon
`1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9` and horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57`.
