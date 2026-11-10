// Game-side glue for the Super Factory Manager port (slice 1).
//
// Injected into `crate::game` as `sfm_state`; it needs the game, so it is not
// part of the standalone library (only `runtime.rs` is).
//
// The manager is a content block (`sfm:manager`) with a mod block entity that
// keeps what upstream keeps on the program disk: the program text and the label
// table (label → positions). Each server tick, after the level's own tick, the
// manager counts up and runs every `EVERY n TICKS` trigger whose
// `tick % n == 0` is due, moving items between the containers its labels reach
// through its cable network. Upstream does this in the block entity's own tick;
// the pin reverts an entity's own writes there, so the port runs it at
// `level_tick_post`, as `moderndynamics` does for its pipes.
//
// Read from Super Factory Manager 4.34.0 (`ManagerBlockEntity.serverTick`,
// `Program.tick`, `InputStatement.gatherSlots`, `OutputStatement.tick`,
// `CableNetwork`, `SFMBlockCapabilityDiscovery`, `LabelPositionHolder`). See
// `../../FINDINGS.md`.

use std::collections::BTreeMap;
use std::sync::Mutex;

use horizon_client::containers::{BlockEntity, ModBlockEntity};
use horizon_client::text::Text;
use horizon_core::geometry::Direction;
use horizon_world::nbt::{Compound, JavaString, Tag};

use crate::game::Game;
use crate::game::native_server::{NativeCommand, NativeServerPlayer};
use crate::game::sfm::{
    Labelled, Move, ParseError, Pos, Program, Slot, Trigger, World, due, plan,
};

/// The manager block (`assets/sfm/blockstates/manager.json`).
pub const MANAGER: &str = "sfm:manager";
/// The plain cable (`assets/sfm/blockstates/cable.json`), a network member.
pub const CABLE: &str = "sfm:cable";
/// The block entity's saved keys. Upstream keeps both on the program disk
/// (`LabelPositionHolder`, `DiskItem`); this slice has no disk, so the manager
/// holds them and the port records the deviation.
const PROGRAM: &str = "Program";
const LABELS: &str = "Labels";
/// Upstream's `Program.MAX_PROGRAM_LENGTH`, and the shorter cap a command line
/// can carry.
const MAX_PROGRAM_LENGTH: usize = 32_000;

/// What the last program tick did, and how far the manager has counted.
#[derive(Debug, Clone, Default)]
struct Entry {
    /// A hash of the program text, so a changed program recompiles.
    hash: u64,
    compiled: Option<Result<Program, ParseError>>,
    /// Upstream's `manager.tick`, which is not saved either.
    tick: u64,
    /// Program ticks that ran, and what the newest one moved.
    fired: u64,
    moved: u64,
    moves: usize,
}

static STATE: Mutex<Option<BTreeMap<(String, Pos), Entry>>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<BTreeMap<(String, Pos), Entry>>> {
    STATE.lock().expect("sfm state lock poisoned")
}

// ---------------------------------------------------------------------------
// The block entity
// ---------------------------------------------------------------------------

/// A placed manager gets a block entity (`native_block_entity`, API 15).
pub fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == MANAGER && entity.is_none() {
        *entity = Some((MANAGER.to_owned(), Compound::new()));
    }
}

fn at(pos: Pos) -> (i32, i32, i32) {
    (pos[0], pos[1], pos[2])
}

/// The position one block along `direction`.
fn step(pos: Pos, direction: Direction) -> Pos {
    let [dx, dy, dz] = direction.normal();
    [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

fn manager_at<'a>(game: &'a Game<'_>, pos: Pos) -> Option<&'a ModBlockEntity> {
    match game.world.block_entities.get(&at(pos)) {
        Some(BlockEntity::Mod(entity)) if entity.id == MANAGER => Some(entity),
        _ => None,
    }
}

fn manager_mut<'a>(game: &'a mut Game<'_>, pos: Pos) -> Option<&'a mut ModBlockEntity> {
    match game.world.block_entities.get_mut(&at(pos)) {
        Some(BlockEntity::Mod(entity)) if entity.id == MANAGER => Some(entity),
        _ => None,
    }
}

fn string(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::String(value)) => Some(value.to_string_lossy()),
        _ => None,
    }
}

/// The program text and label table the manager holds.
fn read_manager(game: &Game<'_>, pos: Pos) -> Option<(String, Vec<Labelled>)> {
    let entity = manager_at(game, pos)?;
    Some((
        string(&entity.nbt, PROGRAM).unwrap_or_default(),
        read_labels(&entity.nbt),
    ))
}

/// `"x,y,z"`, the key a position is saved under.
fn format_pos(pos: Pos) -> String {
    format!("{}, {}, {}", pos[0], pos[1], pos[2])
}

/// Three words (`4 100 0`) as a position.
fn parse_pos_words(words: &[&str]) -> Option<Pos> {
    let x = words.first()?.parse::<i32>().ok()?;
    let y = words.get(1)?.parse::<i32>().ok()?;
    let z = words.get(2)?.parse::<i32>().ok()?;
    Some([x, y, z])
}

fn parse_pos(text: &str) -> Option<Pos> {
    let mut parts = text.split(',').map(|part| part.trim().parse::<i32>().ok());
    let x = parts.next()??;
    let y = parts.next()??;
    let z = parts.next()??;
    Some([x, y, z])
}

/// The label table: one key per label, a list of positions under it.
fn read_labels(nbt: &Compound) -> Vec<Labelled> {
    let Some(Tag::Compound(table)) = nbt.get(&JavaString::from(LABELS)) else {
        return Vec::new();
    };
    let mut labels = Vec::new();
    for (label, positions) in table {
        let Tag::List(entries) = positions else {
            continue;
        };
        for entry in entries {
            let Tag::String(entry) = entry else { continue };
            if let Some(pos) = parse_pos(&entry.to_string_lossy()) {
                labels.push(Labelled {
                    label: label.to_string_lossy(),
                    pos,
                });
            }
        }
    }
    labels
}

fn add_label(nbt: &mut Compound, pos: Pos, label: &str) -> bool {
    let key = JavaString::from(LABELS);
    let mut table = match nbt.get(&key) {
        Some(Tag::Compound(table)) => table.clone(),
        _ => Compound::new(),
    };
    let label_key = JavaString::from(label);
    let mut positions = match table.get(&label_key) {
        Some(Tag::List(entries)) => entries.clone(),
        _ => Vec::new(),
    };
    let text = format_pos(pos);
    if positions
        .iter()
        .any(|entry| matches!(entry, Tag::String(seen) if seen.to_string_lossy() == text))
    {
        return false;
    }
    positions.push(Tag::String(text.as_str().into()));
    table.insert(label_key, Tag::List(positions));
    nbt.insert(key, Tag::Compound(table));
    true
}

fn remove_label(nbt: &mut Compound, pos: Pos, label: &str) -> bool {
    let key = JavaString::from(LABELS);
    let Some(Tag::Compound(mut table)) = nbt.get(&key).cloned() else {
        return false;
    };
    let label_key = JavaString::from(label);
    let Some(Tag::List(positions)) = table.get(&label_key).cloned() else {
        return false;
    };
    let text = format_pos(pos);
    let before = positions.len();
    let kept: Vec<Tag> = positions
        .into_iter()
        .filter(|entry| !matches!(entry, Tag::String(seen) if seen.to_string_lossy() == text))
        .collect();
    if kept.len() == before {
        return false;
    }
    if kept.is_empty() {
        table.remove(&label_key);
    } else {
        table.insert(label_key, Tag::List(kept));
    }
    nbt.insert(key, Tag::Compound(table));
    true
}

// ---------------------------------------------------------------------------
// The tick
// ---------------------------------------------------------------------------

/// After the level's tick: every manager counts up and runs what is due
/// (`native_level_tick_post`, API 23).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_level_tick_post(game: &mut Game<'_>, dimension: &str) {
    let mut positions: Vec<Pos> = game
        .world
        .block_entities
        .iter()
        .filter_map(|(&(x, y, z), entity)| match entity {
            BlockEntity::Mod(entity) if entity.id == MANAGER => Some([x, y, z]),
            _ => None,
        })
        .collect();
    positions.sort_unstable();
    for pos in &positions {
        tick_manager(game, dimension, *pos);
    }
    // A manager that is gone takes its counter with it, as upstream's state
    // died with the block entity.
    {
        let mut guard = state();
        if let Some(state) = guard.as_mut() {
            state.retain(|(level, pos), _| level != dimension || positions.contains(pos));
        }
    }
}

fn hash_text(text: &str) -> u64 {
    // FNV-1a, enough to notice a changed program.
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x1000_0000_01b3)
    })
}

fn tick_manager(game: &mut Game<'_>, dimension: &str, pos: Pos) {
    let Some((text, labels)) = read_manager(game, pos) else {
        return;
    };
    let hash = hash_text(&text);
    let (program, tick) = {
        let mut guard = state();
        let state = guard.get_or_insert_with(BTreeMap::new);
        let entry = state.entry((dimension.to_owned(), pos)).or_default();
        if entry.compiled.is_none() || entry.hash != hash {
            entry.compiled = Some(Program::parse(&text));
            entry.hash = hash;
            entry.fired = 0;
            entry.moved = 0;
            entry.moves = 0;
        }
        entry.tick = entry.tick.saturating_add(1);
        let program = match entry.compiled.as_ref() {
            Some(Ok(program)) => program.clone(),
            _ => return,
        };
        (program, entry.tick)
    };

    // Nothing to do on most ticks: only a due trigger is worth reading the
    // network's containers for.
    let firing: Vec<&Trigger> = program
        .triggers
        .iter()
        .filter(|trigger| due(trigger, tick))
        .collect();
    if firing.is_empty() {
        return;
    }

    let world = snapshot(game, dimension, pos, &labels);
    let mut moves: Vec<Move> = Vec::new();
    for trigger in &firing {
        moves.extend(plan(trigger, &world));
    }
    let fired = firing.len() as u64;
    let moved = execute(game, dimension, &moves);
    let mut guard = state();
    if let Some(entry) = guard
        .as_mut()
        .and_then(|state| state.get_mut(&(dimension.to_owned(), pos)))
    {
        entry.fired += fired;
        entry.moved = moved;
        entry.moves = moves.len();
    }
}

/// The positions the manager's cable network reaches: its cables and managers,
/// and the blocks touching them. Upstream's `isAdjacentToCable` refuses
/// everything else, so a labelled chest with no cable beside it is invisible.
fn network(game: &Game<'_>, manager: Pos) -> Vec<Pos> {
    let mut members: Vec<Pos> = vec![manager];
    let mut index = 0;
    while index < members.len() {
        let current = members[index];
        index += 1;
        for direction in Direction::ALL {
            let beside = step(current, direction);
            if members.contains(&beside) {
                continue;
            }
            let block = game.native_block(beside);
            if block == CABLE || block == MANAGER {
                members.push(beside);
            }
        }
    }
    let mut reachable = members.clone();
    for member in &members {
        for direction in Direction::ALL {
            let beside = step(*member, direction);
            if !reachable.contains(&beside) {
                reachable.push(beside);
            }
        }
    }
    reachable.sort_unstable();
    reachable
}

/// The world one program tick sees: what the network reaches, the manager's
/// labels, and the slots of every labelled container among them.
fn snapshot(game: &Game<'_>, dimension: &str, pos: Pos, labels: &[Labelled]) -> World {
    let reachable = network(game, pos);
    let mut slots: Vec<Slot> = Vec::new();
    let mut seen: Vec<Pos> = Vec::new();
    for entry in labels {
        if !reachable.contains(&entry.pos) || seen.contains(&entry.pos) {
            continue;
        }
        seen.push(entry.pos);
        for side in 0..6usize {
            let Some(found) = game.native_container_slots(dimension, entry.pos, Direction::ALL[side])
            else {
                continue;
            };
            for slot in found {
                let (item, count) = match &slot.stack {
                    Some(stack) => (stack.item.to_string(), stack.count as u64),
                    None => (String::new(), 0),
                };
                slots.push(Slot {
                    pos: entry.pos,
                    side: side as u8,
                    index: slot.slot,
                    item,
                    count,
                    can_take: slot.can_take,
                });
            }
        }
    }
    slots.sort_by_key(|slot| (slot.pos, slot.side, slot.index));
    World {
        reachable,
        labels: labels.to_vec(),
        slots,
    }
}

/// Runs the plan through `native_move_items`, one item at a time, so nothing is
/// lost when a target refuses. Answers how many items moved.
fn execute(game: &mut Game<'_>, dimension: &str, moves: &[Move]) -> u64 {
    let mut moved = 0u64;
    for step in moves {
        let max = u32::try_from(step.max).unwrap_or(u32::MAX);
        let count = game.native_move_items(
            dimension,
            step.from,
            Direction::ALL[usize::from(step.from_side)],
            step.to,
            Direction::ALL[usize::from(step.to_side)],
            Some(&step.item),
            max,
        );
        moved += u64::from(count);
    }
    moved
}

// ---------------------------------------------------------------------------
// The `/sfm` surface
// ---------------------------------------------------------------------------

/// The commands (`native_register_commands`, API 8). Upstream has no commands:
/// its program editor is a GUI on the disk, which this slice does not port, so
/// the program and the labels are set by command (precedent:
/// `/charginggadgets item`, `/ftbultimine shape`).
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op("sfm program <pos:block_pos> <program:text>", 2));
    commands.push(NativeCommand::new("sfm show <pos:block_pos>"));
    commands.push(NativeCommand::op(
        "sfm label <pos:block_pos> <target:block_pos> <name:text>",
        2,
    ));
    commands.push(NativeCommand::op(
        "sfm unlabel <pos:block_pos> <target:block_pos> <name:text>",
        2,
    ));
    commands.push(NativeCommand::new("sfm labels <pos:block_pos>"));
    commands.push(NativeCommand::op("sfm tick <pos:block_pos> <value:integer>", 2));
}

/// `/sfm program|show|label|unlabel|labels|tick` (`native_perform_command`,
/// API 8).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_perform_command(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let words: Vec<&str> = command.split_whitespace().collect();
    if words.first() != Some(&"sfm") {
        return;
    }
    *ran = true;
    let usage = Text::literal(
        "usage: /sfm program|show|label|unlabel|labels|tick <x> <y> <z> …",
    );
    let Some(subcommand) = words.get(1).copied() else {
        game.native_feedback(usage);
        return;
    };
    let Some(pos) = words.get(2..5).and_then(parse_pos_words) else {
        game.native_feedback(usage);
        return;
    };
    if manager_at(game, pos).is_none() {
        game.native_feedback(Text::literal(format!(
            "no {} at {}",
            MANAGER,
            format_pos(pos)
        )));
        return;
    }
    let rest: Vec<&str> = words.get(5..).map(<[&str]>::to_vec).unwrap_or_default();
    match subcommand {
        "program" => {
            let text = rest.join(" ").replace(';', "\n");
            if text.trim().is_empty() {
                game.native_feedback(Text::literal("usage: /sfm program <x> <y> <z> <program>"));
                return;
            }
            if text.len() > MAX_PROGRAM_LENGTH {
                game.native_feedback(Text::literal(format!(
                    "the program is {} characters; the limit is {MAX_PROGRAM_LENGTH}",
                    text.len()
                )));
                return;
            }
            let compiled = Program::parse(&text);
            // The program is stored as typed, as upstream keeps it on the disk;
            // an unreadable one makes the manager report its state, not crash.
            set_program(game, pos, &text);
            match compiled {
                Ok(program) => game.native_feedback(Text::literal(format!(
                    "program set: {} trigger(s) at {}",
                    program.triggers.len(),
                    format_pos(pos)
                ))),
                Err(error) => game.native_feedback(Text::literal(format!(
                    "program set, but it does not compile: {error}"
                ))),
            }
        }
        "show" => show(game, &player.dimension, pos),
        "label" | "unlabel" => {
            // The manager's position comes first, then the block to label (the
            // label gun's click, which a command cannot make).
            let Some(target) = words.get(5..8).and_then(parse_pos_words) else {
                game.native_feedback(Text::literal(format!(
                    "usage: /sfm {subcommand} <manager x y z> <block x y z> <name>"
                )));
                return;
            };
            let name = words.get(8..).map(|words| words.join(" ")).unwrap_or_default();
            if name.is_empty() {
                game.native_feedback(Text::literal(format!(
                    "usage: /sfm {subcommand} <manager x y z> <block x y z> <name>"
                )));
                return;
            }
            let changed = match manager_mut(game, pos) {
                Some(entity) if subcommand == "label" => add_label(&mut entity.nbt, target, &name),
                Some(entity) => remove_label(&mut entity.nbt, target, &name),
                None => false,
            };
            if changed {
                game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
            }
            game.native_feedback(Text::literal(if changed {
                format!(
                    "label {name:?} {} {}",
                    if subcommand == "label" { "on" } else { "off" },
                    format_pos(target)
                )
            } else {
                format!(
                    "label {name:?} was already {}",
                    if subcommand == "label" { "on" } else { "off" }
                )
            }));
        }
        "labels" => labels(game, pos),
        "tick" => {
            let value = rest.first().and_then(|word| word.parse::<u64>().ok());
            let Some(value) = value else {
                game.native_feedback(Text::literal("usage: /sfm tick <x> <y> <z> <value>"));
                return;
            };
            let mut guard = state();
            let state = guard.get_or_insert_with(BTreeMap::new);
            let entry = state
                .entry((player.dimension.clone(), pos))
                .or_default();
            entry.tick = value;
            drop(guard);
            game.native_feedback(Text::literal(format!(
                "tick set to {value} at {}",
                format_pos(pos)
            )));
        }
        _ => game.native_feedback(usage),
    }
}

fn set_program(game: &mut Game<'_>, pos: Pos, text: &str) {
    if let Some(entity) = manager_mut(game, pos) {
        entity
            .nbt
            .insert(JavaString::from(PROGRAM), Tag::String(text.into()));
    }
    game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
}

/// The state line `/sfm show` prints: `key=value` pairs the harness parses.
fn show(game: &mut Game<'_>, dimension: &str, pos: Pos) {
    let Some((text, labels)) = read_manager(game, pos) else {
        return;
    };
    let hash = hash_text(&text);
    let (entry, program) = {
        let mut guard = state();
        let state = guard.get_or_insert_with(BTreeMap::new);
        let entry = state.entry((dimension.to_owned(), pos)).or_default();
        if entry.compiled.is_none() || entry.hash != hash {
            entry.compiled = Some(Program::parse(&text));
            entry.hash = hash;
        }
        (entry.clone(), entry.compiled.as_ref().and_then(|c| c.as_ref().ok().cloned()))
    };
    let state_word = if text.trim().is_empty() {
        "no_program"
    } else if program.is_none() {
        "invalid"
    } else {
        "running"
    };
    let triggers = program.as_ref().map_or(0, |program: &Program| program.triggers.len());
    game.native_feedback(Text::literal(format!(
        "SFM pos={} tick={} state={} triggers={} labels={} fired={} moved={} moves={}",
        format_pos(pos),
        entry.tick,
        state_word,
        triggers,
        labels.len(),
        entry.fired,
        entry.moved,
        entry.moves,
    )));
    if let Some(Err(error)) = entry.compiled.as_ref() {
        game.native_feedback(Text::literal(format!("program error: {error}")));
    }
}

/// `/sfm labels`: the manager's label table, one label per line.
fn labels(game: &mut Game<'_>, pos: Pos) {
    let Some((_, labels)) = read_manager(game, pos) else {
        return;
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in labels {
        *counts.entry(entry.label).or_default() += 1;
    }
    if counts.is_empty() {
        game.native_feedback(Text::literal(format!("no labels at {}", format_pos(pos))));
        return;
    }
    for (label, count) in counts {
        game.native_feedback(Text::literal(format!("label {label:?}: {count} block(s)")));
    }
}
