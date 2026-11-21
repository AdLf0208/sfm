//! Game-side glue for the Super Factory Manager port (slice 1).
//!
//! The rules — the SFML parser, the timer predicate, the plan one trigger makes and the
//! retention accounting — live in [`crate::runtime`] and are unit-tested without a world.
//! This file is everything that needs the game: the manager's block entity, its tick, the
//! cable network, the container reads, the item moves and the `/sfm` commands.
//!
//! The manager is a content block (`sfm:manager`) with a declared block entity that keeps
//! what upstream keeps on the program disk: the program text and the label table
//! (label → positions). Each server tick the manager's block entity **queues itself**
//! (`block_entity_tick`), and the moves run once at `level_tick_post`, because the SDK cannot
//! enumerate a level's block entities and a block entity that is out of the world during its
//! own tick is a bad place to walk a cable network. Upstream does it in the block entity's
//! own tick; the 0.1.0 port did the same at `level_tick_post` and the queue keeps that shape.
//!
//! Read from Super Factory Manager 4.34.0 (`ManagerBlockEntity.serverTick`, `Program.tick`,
//! `InputStatement.gatherSlots`, `OutputStatement.tick`, `CableNetwork`,
//! `SFMBlockCapabilityDiscovery`, `LabelPositionHolder`). See `../../FINDINGS.md`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::runtime::{
    Labelled, MAX_NETWORK_VISITS, Move, ParseError, Pos, Program, Slot, Trigger, World,
    diagnostics_permitted, due, plan, settle, walk_members,
};
use mod_api::{
    BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey, BlockPart, Capability,
    Command, CommandArgumentValue, CommandInvocation, Compound, Direction, Flow, Host,
    HostTransaction, ITEMS, ItemResource, LevelRef, RegisterCommandsEvent, Registrar,
    ResourceHandler, RunModCommandEvent, ServerHost, Tag, Text, Transaction,
};

/// The manager block (`assets/sfm/block/manager.json` read as a template declaration).
pub(crate) const MANAGER: &str = "sfm:manager";
/// The plain cable (`assets/sfm/cable`), a network member.
pub(crate) const CABLE: &str = "sfm:cable";
/// The block entity type this mod declares for the manager (its own namespace).
const MANAGER_TYPE: &str = "factory_manager";
/// The block entity's saved keys. Upstream keeps both on the program disk
/// (`LabelPositionHolder`, `DiskItem`); this slice has no disk, so the manager holds them
/// and the port records the deviation.
const PROGRAM: &str = "Program";
const LABELS: &str = "Labels";
/// Upstream's `Program.MAX_PROGRAM_LENGTH`, and the shorter cap a command line can carry.
const MAX_PROGRAM_LENGTH: usize = 32_000;

/// The registration keys, so a hot tick compares block identity instead of allocating a
/// string per block read. Registration runs before any hook.
static MANAGER_KEY: OnceLock<BlockKey> = OnceLock::new();
static CABLE_KEY: OnceLock<BlockKey> = OnceLock::new();

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

/// One manager's state, keyed by its level and position. `LevelRef` is `Eq + Hash` but not
/// ordered, so this is a `HashMap`; nothing iterates it for output.
type Key = (LevelRef, Pos);
static STATE: Mutex<Option<HashMap<Key, Entry>>> = Mutex::new(None);

/// The managers whose block entity ticked this server tick, waiting for `level_tick_post`.
static QUEUED: Mutex<Vec<(LevelRef, Pos)>> = Mutex::new(Vec::new());

fn state() -> MutexGuard<'static, Option<HashMap<Key, Entry>>> {
    STATE.lock().expect("sfm state lock poisoned")
}

// ---------------------------------------------------------------------------
// The block entity: ticks queue, `level_tick_post` runs
// ---------------------------------------------------------------------------

/// The manager's block entity Part: it ticks, and its tick only queues the position.
pub(crate) struct ManagerPart;

#[mod_api::horizon_part]
impl BlockPart for ManagerPart {}

#[mod_api::horizon_part]
impl BlockEntityPart for ManagerPart {
    /// The manager's block entity ticks (`EntityBlock.getTicker`).
    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// Queue the manager for this tick's pass and change nothing of its own payload.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        if let Some(level) = level_in_place(host) {
            let mut queue = QUEUED.lock().expect("sfm tick queue poisoned");
            queue.push((level, event.pos));
        }
        *event.reply = mod_api::BlockEntityTickReply::Unchanged;
    }
}

/// Declare the manager's block entity type (held by the manager block) and attach the Part.
pub(crate) fn register(registrar: &mut dyn Registrar) {
    let manager = registrar.block_key(MANAGER);
    let cable = registrar.block_key(CABLE);
    let _ = MANAGER_KEY.set(manager);
    let _ = CABLE_KEY.set(cable);
    let kind = registrar
        .block_entity_type(MANAGER_TYPE)
        .blocks(&[MANAGER])
        .build();
    let _ = registrar.block_entity_part(&kind, ManagerPart);
}

/// The level whose block entities the server ticks now. A block-entity tick is not told its
/// level; the level in place is the one being ticked.
fn level_in_place(host: &dyn ServerHost) -> Option<LevelRef> {
    host.levels()
        .list_levels()
        .into_iter()
        .find(|level| level.in_place(host))
}

/// After the level's tick: every manager that ticked counts up and runs what is due.
pub(crate) fn on_level_tick_post(host: &mut dyn ServerHost, level: LevelRef) {
    let mut positions: Vec<Pos> = {
        let mut queue = QUEUED.lock().expect("sfm tick queue poisoned");
        let all = std::mem::take(&mut *queue);
        let mut here = Vec::new();
        for (queued_level, pos) in all {
            if queued_level == level {
                here.push(pos);
            } else {
                queue.push((queued_level, pos));
            }
        }
        here
    };
    positions.sort_unstable();
    positions.dedup();
    for pos in &positions {
        tick_manager(host, level, *pos);
    }
    // A manager that is gone takes its counter with it, as upstream's state died with the
    // block entity.
    let mut guard = state();
    if let Some(entries) = guard.as_mut() {
        entries.retain(|(entry_level, pos), _| *entry_level != level || positions.contains(pos));
    }
}

fn tick_manager(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) {
    let Some(data) = host
        .block_entity(level, pos)
        .and_then(|entity| entity.data(host))
    else {
        return;
    };
    let text = program_text(&data);
    let labels = read_labels(&data);
    let hash = hash_text(&text);
    let (program, tick) = {
        let mut guard = state();
        let entries = guard.get_or_insert_with(HashMap::new);
        let entry = entries.entry((level, pos)).or_default();
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

    // Nothing to do on most ticks: only a due trigger is worth reading the network's
    // containers for.
    let firing: Vec<&Trigger> = program
        .triggers
        .iter()
        .filter(|trigger| due(trigger, tick))
        .collect();
    if firing.is_empty() {
        return;
    }

    let world = snapshot(host, level, pos, &labels);
    let mut moves: Vec<Move> = Vec::new();
    for trigger in &firing {
        moves.extend(plan(trigger, &world));
    }
    let fired = firing.len() as u64;
    let moved = execute(host, level, &moves);
    let mut guard = state();
    if let Some(entry) = guard
        .as_mut()
        .and_then(|entries| entries.get_mut(&(level, pos)))
    {
        entry.fired += fired;
        entry.moved = moved;
        entry.moves = moves.len();
    }
}

// ---------------------------------------------------------------------------
// The network and the world one program tick sees
// ---------------------------------------------------------------------------

/// The position one block along `direction`.
fn step(pos: Pos, direction: Direction) -> Pos {
    let [dx, dy, dz] = direction.normal();
    [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

/// Whether `pos` holds a manager or a cable.
fn is_member(host: &dyn Host, level: LevelRef, pos: Pos) -> bool {
    let Some(found) = level.block(host, pos) else {
        return false;
    };
    MANAGER_KEY.get().is_some_and(|key| *key == found)
        || CABLE_KEY.get().is_some_and(|key| *key == found)
}

/// Whether `pos` holds the block registered under `key`.
fn is_block(host: &dyn Host, level: LevelRef, pos: Pos, key: &OnceLock<BlockKey>) -> bool {
    let Some(found) = level.block(host, pos) else {
        return false;
    };
    key.get().is_some_and(|wanted| *wanted == found)
}

/// The positions the manager's cable network reaches: its cables and managers, and the
/// blocks touching them. Upstream's `isAdjacentToCable` refuses everything else, so a
/// labelled chest with no cable beside it is invisible.
fn network(host: &dyn Host, level: LevelRef, manager: Pos) -> Vec<Pos> {
    // A set walk with a visit budget: the membership test is asked once per position and a
    // player-built network cannot turn one tick into an unbounded read (the review finding).
    let members = walk_members(manager, MAX_NETWORK_VISITS, |pos| {
        is_member(host, level, pos)
    });
    let mut reachable: std::collections::BTreeSet<Pos> = std::collections::BTreeSet::new();
    for member in &members {
        reachable.insert(*member);
        for direction in Direction::ALL {
            reachable.insert(step(*member, direction));
        }
    }
    reachable.into_iter().collect()
}

/// The world one program tick sees: what the network reaches, the manager's labels, and the
/// slots of every labelled container among them.
fn snapshot(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, labels: &[Labelled]) -> World {
    let reachable = network(&*host, level, pos);
    let reachable_set: std::collections::BTreeSet<Pos> = reachable.iter().copied().collect();
    let mut slots: Vec<Slot> = Vec::new();
    let mut seen: std::collections::BTreeSet<Pos> = std::collections::BTreeSet::new();
    for entry in labels {
        if !reachable_set.contains(&entry.pos) || !seen.insert(entry.pos) {
            continue;
        }
        for side in 0..6usize {
            let direction = Direction::ALL[side];
            let Some(handler) = level.capability(host, &ITEMS, entry.pos, Some(direction)) else {
                continue;
            };
            for index in 0..handler.size(host) {
                // Every slot the side reaches is recorded, empty ones included: an empty
                // destination slot is exactly what an OUTPUT needs (its side is otherwise
                // invisible, so a program could not fill a fresh chest). The item of an empty
                // slot is the empty id, which matches no filter and is skipped by every gather.
                let resource = handler.resource(host, index);
                let item = resource
                    .as_ref()
                    .map(|resource| resource.item().id(&*host))
                    .unwrap_or_default();
                let count = u64::from(handler.amount(host, index));
                slots.push(Slot {
                    pos: entry.pos,
                    side: side as u8,
                    index,
                    item,
                    count,
                    can_take: resource.is_some() && count > 0,
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

// ---------------------------------------------------------------------------
// The moves
// ---------------------------------------------------------------------------

/// Runs the plan's moves in one transaction per program tick: what a target refuses goes
/// back into the source slot, so a move never loses an item. Answers how many items moved.
///
/// A remainder the source can no longer take aborts the whole tick: [`settle`] answers `Err`,
/// the transaction rolls back, and nothing leaves any source (the review finding).
fn execute(host: &mut dyn ServerHost, level: LevelRef, moves: &[Move]) -> u64 {
    if moves.is_empty() {
        return 0;
    }
    host.transaction(|host, tx| {
        let mut moved = 0u64;
        for step in moves {
            moved += run_move(host, tx, level, step)?;
        }
        Ok::<u64, ()>(moved)
    })
    .unwrap_or(0)
}

/// One move: up to `step.max` of `step.item` out of the source side into the target side.
fn run_move(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    step: &Move,
) -> Result<u64, ()> {
    let max = u32::try_from(step.max).unwrap_or(u32::MAX);
    if max == 0 {
        return Ok(0);
    }
    let from = Direction::ALL[usize::from(step.from_side)];
    let to = Direction::ALL[usize::from(step.to_side)];
    let Some(source) = level.capability(host, &ITEMS, step.from, Some(from)) else {
        return Ok(0);
    };
    let Some((index, resource)) = first_of(host, &source, &step.item) else {
        return Ok(0);
    };
    let Some(target) = level.capability(host, &ITEMS, step.to, Some(to)) else {
        return Ok(0);
    };
    let taken = source.extract(host, tx, index, &resource, max);
    if taken == 0 {
        return Ok(0);
    }
    let put = target.insert_any(host, tx, &resource, taken);
    if put == taken {
        return Ok(u64::from(put));
    }
    // Give the refused part back: the slot it came from has room for it again, and the whole
    // program tick is one transaction. A source that cannot take it back is a lost item, so
    // the tick is rolled back instead of committed.
    let returned = source.insert(host, tx, index, &resource, taken - put);
    Ok(u64::from(settle(taken, put, returned).map_err(|_| ())?))
}

/// The first slot of `handler` holding `item`, and the resource it holds.
fn first_of(
    host: &mut dyn ServerHost,
    handler: &Capability<dyn ResourceHandler<ItemResource>>,
    item: &str,
) -> Option<(usize, ItemResource)> {
    for index in 0..handler.size(host) {
        let Some(resource) = handler.resource(host, index) else {
            continue;
        };
        if resource.item().id(&*host) == item {
            return Some((index, resource));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The `/sfm` surface
// ---------------------------------------------------------------------------

/// The commands. Upstream has no commands: its program editor is a GUI on the disk, which
/// this slice does not port, so the program and the labels are set by command (precedent:
/// `/charginggadgets item`, `/ftbultimine shape`).
pub(crate) fn declare_commands(event: &mut RegisterCommandsEvent<'_>) {
    event
        .commands
        .push(Command::op("sfm program <pos:block_pos> <program:text>", 2));
    event
        .commands
        .push(Command::new("sfm show <pos:block_pos>"));
    event.commands.push(Command::op(
        "sfm label <pos:block_pos> <target:block_pos> <name:text>",
        2,
    ));
    event.commands.push(Command::op(
        "sfm unlabel <pos:block_pos> <target:block_pos> <name:text>",
        2,
    ));
    event
        .commands
        .push(Command::new("sfm labels <pos:block_pos>"));
    event
        .commands
        .push(Command::op("sfm tick <pos:block_pos> <value:integer>", 2));
}

const USAGE: &str = "usage: /sfm program|show|label|unlabel|labels|tick <x> <y> <z> …";

/// `/sfm program|show|label|unlabel|labels|tick`, with the arguments the command tree
/// resolved. Answers `Flow::Cancel` when it handled one.
pub(crate) fn run_command(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) -> Flow {
    let invocation = event.command;
    if invocation.literals.first().map(String::as_str) != Some("sfm") {
        return Flow::Continue;
    }
    let Some(subcommand) = invocation.literals.get(1).map(String::as_str) else {
        feedback(host, USAGE);
        return Flow::Cancel;
    };
    let Some(CommandArgumentValue::BlockPos(pos)) = invocation.argument("pos") else {
        feedback(host, USAGE);
        return Flow::Cancel;
    };
    let pos = *pos;
    let Some(level) = level_of(host, invocation) else {
        feedback(host, format!("no {MANAGER} at {}", format_pos(pos)));
        return Flow::Cancel;
    };
    if !manager_at(host, level, pos) {
        feedback(host, format!("no {MANAGER} at {}", format_pos(pos)));
        return Flow::Cancel;
    }
    // `show` and `labels` report the manager's counters and its label table, which are its
    // owner's build, so they are a nearby read or an operator's, not a remote probe.
    if matches!(subcommand, "show" | "labels") && !diagnostics_allowed(host, pos) {
        feedback(host, INFO_DENIED);
        return Flow::Cancel;
    }
    match subcommand {
        "program" => {
            let text = match invocation.argument("program") {
                Some(CommandArgumentValue::String(text)) => text.replace(';', "\n"),
                _ => String::new(),
            };
            if text.trim().is_empty() {
                feedback(host, "usage: /sfm program <x> <y> <z> <program>");
                return Flow::Cancel;
            }
            if text.len() > MAX_PROGRAM_LENGTH {
                feedback(
                    host,
                    format!(
                        "the program is {} characters; the limit is {MAX_PROGRAM_LENGTH}",
                        text.len()
                    ),
                );
                return Flow::Cancel;
            }
            let compiled = Program::parse(&text);
            // The program is stored as typed, as upstream keeps it on the disk; an
            // unreadable one makes the manager report its state, not crash.
            set_program(host, level, pos, &text);
            match compiled {
                Ok(program) => feedback(
                    host,
                    format!(
                        "program set: {} trigger(s) at {}",
                        program.triggers.len(),
                        format_pos(pos)
                    ),
                ),
                Err(error) => feedback(
                    host,
                    format!("program set, but it does not compile: {error}"),
                ),
            }
        }
        "show" => show(host, level, pos),
        "label" | "unlabel" => {
            // The manager's position comes first, then the block to label (the label gun's
            // click, which a command cannot make).
            let target = match invocation.argument("target") {
                Some(CommandArgumentValue::BlockPos(target)) => *target,
                _ => {
                    feedback(
                        host,
                        format!("usage: /sfm {subcommand} <manager x y z> <block x y z> <name>"),
                    );
                    return Flow::Cancel;
                }
            };
            let name = match invocation.argument("name") {
                Some(CommandArgumentValue::String(name)) => name.trim().to_owned(),
                _ => String::new(),
            };
            if name.is_empty() {
                feedback(
                    host,
                    format!("usage: /sfm {subcommand} <manager x y z> <block x y z> <name>"),
                );
                return Flow::Cancel;
            }
            let adding = subcommand == "label";
            let changed = edit_manager(host, level, pos, |data| {
                if adding {
                    add_label(data, target, &name)
                } else {
                    remove_label(data, target, &name)
                }
            });
            let state = if adding { "on" } else { "off" };
            feedback(
                host,
                if changed {
                    format!("label {name:?} {state} {}", format_pos(target))
                } else {
                    format!("label {name:?} was already {state}")
                },
            );
        }
        "labels" => labels_readout(host, level, pos),
        "tick" => {
            let value = match invocation.argument("value") {
                Some(CommandArgumentValue::Integer(value)) => u64::try_from(*value).ok(),
                _ => None,
            };
            let Some(value) = value else {
                feedback(host, "usage: /sfm tick <x> <y> <z> <value>");
                return Flow::Cancel;
            };
            let mut guard = state();
            let entries = guard.get_or_insert_with(HashMap::new);
            let entry = entries.entry((level, pos)).or_default();
            entry.tick = value;
            drop(guard);
            feedback(host, format!("tick set to {value} at {}", format_pos(pos)));
        }
        _ => feedback(host, USAGE),
    }
    Flow::Cancel
}

/// The refusal `/sfm show` and `/sfm labels` answer a source without reach or permission.
const INFO_DENIED: &str =
    "sfm: too far away; stand within 6 blocks of the manager or ask an operator";

/// Whether the command's source may read a manager at `pos`: the acting player needs reach or
/// an operator's permission; the console may read anywhere.
fn diagnostics_allowed(host: &dyn ServerHost, pos: Pos) -> bool {
    let Some(player) = host.players().acting_player() else {
        return diagnostics_permitted(None, None);
    };
    let permission = player.permission_level(host);
    let distance = player
        .entity()
        .distance_to_sqr(
            host,
            [
                f64::from(pos[0]) + 0.5,
                f64::from(pos[1]) + 0.5,
                f64::from(pos[2]) + 0.5,
            ],
        )
        .map(f64::sqrt);
    diagnostics_permitted(permission, distance)
}

/// Whether `pos` holds the manager block with its block entity.
fn manager_at(host: &dyn ServerHost, level: LevelRef, pos: Pos) -> bool {
    is_block(host, level, pos, &MANAGER_KEY) && host.block_entity(level, pos).is_some()
}

fn set_program(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, text: &str) {
    edit_manager(host, level, pos, |data| {
        data.insert(PROGRAM.into(), Tag::String(text.into()));
        true
    });
}

/// Read the manager's payload, let `edit` change it, and write it back as a whole: the
/// label table has to lose keys, and `merge_data` is a recursive merge that cannot delete
/// one (`/data merge block`). The payload is the manager's own — `Program` and `Labels`.
fn edit_manager(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    edit: impl FnOnce(&mut Compound) -> bool,
) -> bool {
    let Some(entity) = host.block_entity(level, pos) else {
        return false;
    };
    let Some(mut data) = entity.data(host) else {
        return false;
    };
    if !edit(&mut data) {
        return false;
    }
    entity.replace_data(host, data).unwrap_or(false)
}

/// The state line `/sfm show` prints: `key=value` pairs the harness parses.
fn show(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) {
    let Some(data) = host
        .block_entity(level, pos)
        .and_then(|entity| entity.data(host))
    else {
        return;
    };
    let text = program_text(&data);
    let labels = read_labels(&data);
    // Diagnostics for the harness: what the network reaches and how many slots it saw.
    let world = snapshot(host, level, pos, &labels);
    let reachable = world.reachable.len();
    let slots = world.slots.len();
    let hash = hash_text(&text);
    let (entry, program) = {
        let mut guard = state();
        let entries = guard.get_or_insert_with(HashMap::new);
        let entry = entries.entry((level, pos)).or_default();
        if entry.compiled.is_none() || entry.hash != hash {
            entry.compiled = Some(Program::parse(&text));
            entry.hash = hash;
        }
        (
            entry.clone(),
            entry
                .compiled
                .as_ref()
                .and_then(|compiled| compiled.as_ref().ok().cloned()),
        )
    };
    let state_word = if text.trim().is_empty() {
        "no_program"
    } else if program.is_none() {
        "invalid"
    } else {
        "running"
    };
    let triggers = program
        .as_ref()
        .map_or(0, |program: &Program| program.triggers.len());
    feedback(
        host,
        format!(
            "SFM pos={} tick={} state={} triggers={} labels={} fired={} moved={} moves={} reachable={reachable} slots={slots}",
            format_pos(pos),
            entry.tick,
            state_word,
            triggers,
            labels.len(),
            entry.fired,
            entry.moved,
            entry.moves,
        ),
    );
    if let Some(Err(error)) = entry.compiled.as_ref() {
        feedback(host, format!("program error: {error}"));
    }
}

/// `/sfm labels`: the manager's label table, one label per line.
fn labels_readout(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) {
    let Some(data) = host
        .block_entity(level, pos)
        .and_then(|entity| entity.data(host))
    else {
        return;
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in read_labels(&data) {
        *counts.entry(entry.label).or_default() += 1;
    }
    if counts.is_empty() {
        feedback(host, format!("no labels at {}", format_pos(pos)));
        return;
    }
    for (label, count) in counts {
        feedback(host, format!("label {label:?}: {count} block(s)"));
    }
}

/// The level a command executes in, from the dimension its invocation named.
fn level_of(host: &dyn ServerHost, invocation: &CommandInvocation) -> Option<LevelRef> {
    invocation.dimension.id(host).and_then(|id| host.level(&id))
}

fn feedback(host: &mut dyn ServerHost, line: impl Into<String>) {
    let _ = host.commands().feedback(Text::literal(line.into()));
}

// ---------------------------------------------------------------------------
// The saved compound
// ---------------------------------------------------------------------------

/// `key`'s tag, with Java's UTF-16 key spelling.
fn tag<'a>(nbt: &'a Compound, key: &str) -> Option<&'a Tag> {
    let units: Vec<u16> = key.encode_utf16().collect();
    nbt.iter()
        .find(|(name, _)| name.0 == units)
        .map(|(_, tag)| tag)
}

/// The program text the manager holds (`Program`), or an empty program.
pub(crate) fn program_text(nbt: &Compound) -> String {
    match tag(nbt, PROGRAM) {
        Some(Tag::String(text)) => text.to_string_lossy(),
        _ => String::new(),
    }
}

/// `"x, y, z"`, the key a position is saved under.
pub(crate) fn format_pos(pos: Pos) -> String {
    format!("{}, {}, {}", pos[0], pos[1], pos[2])
}

/// Three comma-separated numbers (`"4, 100, 0"`) as a position.
fn parse_pos(text: &str) -> Option<Pos> {
    let mut parts = text.split(',').map(|part| part.trim().parse::<i32>().ok());
    let x = parts.next()??;
    let y = parts.next()??;
    let z = parts.next()??;
    if parts.next().is_some() {
        return None;
    }
    Some([x, y, z])
}

/// The label table as (label, saved positions) pairs, in the table's own (sorted) order.
/// The entries are owned strings so the table can be rebuilt whole: a label that loses its
/// last position has to disappear, and a `Compound` has no key type a mod can name.
fn table_entries(nbt: &Compound) -> Vec<(String, Vec<String>)> {
    let Some(Tag::Compound(table)) = tag(nbt, LABELS) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for (label, positions) in table {
        let Tag::List(list) = positions else {
            continue;
        };
        let positions: Vec<String> = list
            .iter()
            .filter_map(|entry| match entry {
                Tag::String(text) => Some(text.to_string_lossy()),
                _ => None,
            })
            .collect();
        entries.push((label.to_string_lossy(), positions));
    }
    entries
}

/// Write the whole label table back, replacing the key's value.
fn write_table(nbt: &mut Compound, entries: &[(String, Vec<String>)]) {
    let mut table = Compound::new();
    for (label, positions) in entries {
        let list: Vec<Tag> = positions
            .iter()
            .map(|text| Tag::String(text.as_str().into()))
            .collect();
        table.insert(label.as_str().into(), Tag::List(list));
    }
    nbt.insert(LABELS.into(), Tag::Compound(table));
}

/// The label table: one key per label, a list of positions under it.
pub(crate) fn read_labels(nbt: &Compound) -> Vec<Labelled> {
    let mut labels = Vec::new();
    for (label, positions) in table_entries(nbt) {
        for saved in positions {
            if let Some(pos) = parse_pos(&saved) {
                labels.push(Labelled {
                    label: label.clone(),
                    pos,
                });
            }
        }
    }
    labels
}

/// Put `pos` under `label`. False when that label already names it.
pub(crate) fn add_label(nbt: &mut Compound, pos: Pos, label: &str) -> bool {
    let mut entries = table_entries(nbt);
    let text = format_pos(pos);
    match entries.iter_mut().find(|(name, _)| name == label) {
        Some((_, positions)) => {
            if positions.contains(&text) {
                return false;
            }
            positions.push(text);
        }
        None => entries.push((label.to_owned(), vec![text])),
    }
    write_table(nbt, &entries);
    true
}

/// Take `pos` off `label`; a label with nothing left goes away. False when it did not name it.
pub(crate) fn remove_label(nbt: &mut Compound, pos: Pos, label: &str) -> bool {
    let mut entries = table_entries(nbt);
    let text = format_pos(pos);
    let Some((_, positions)) = entries.iter_mut().find(|(name, _)| name == label) else {
        return false;
    };
    let before = positions.len();
    positions.retain(|seen| *seen != text);
    if positions.len() == before {
        return false;
    }
    if positions.is_empty() {
        entries.retain(|(name, _)| name != label);
    }
    write_table(nbt, &entries);
    true
}

/// FNV-1a of the program text, enough to notice a changed program.
pub(crate) fn hash_text(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x1000_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compound() -> Compound {
        Compound::new()
    }

    #[test]
    fn a_position_round_trips_through_its_saved_form() {
        assert_eq!(format_pos([4, 100, -7]), "4, 100, -7");
        assert_eq!(parse_pos("4, 100, -7"), Some([4, 100, -7]));
        assert_eq!(parse_pos("4,100,-7"), Some([4, 100, -7]));
        assert_eq!(parse_pos("4, 100"), None);
        assert_eq!(parse_pos("4, 100, x"), None);
    }

    #[test]
    fn the_program_text_is_read_from_its_key_and_an_empty_manager_reads_empty() {
        let mut nbt = compound();
        assert_eq!(program_text(&nbt), "");
        nbt.insert(PROGRAM.into(), Tag::String("EVERY 20 TICKS DO\nEND".into()));
        assert_eq!(program_text(&nbt), "EVERY 20 TICKS DO\nEND");
        // A key holding another tag is not a program, not a crash.
        nbt.insert(PROGRAM.into(), Tag::Int(3));
        assert_eq!(program_text(&nbt), "");
    }

    #[test]
    fn a_label_is_added_once_and_read_back_with_its_position() {
        let mut nbt = compound();
        assert!(add_label(&mut nbt, [4, 100, 0], "a"));
        assert_eq!(
            read_labels(&nbt),
            vec![Labelled {
                label: "a".to_owned(),
                pos: [4, 100, 0]
            }]
        );
        // The same position under the same label is already there.
        assert!(!add_label(&mut nbt, [4, 100, 0], "a"));
        // The same label on another position is a second entry.
        assert!(add_label(&mut nbt, [6, 100, 0], "a"));
        assert_eq!(read_labels(&nbt).len(), 2);
        // A different label keeps its own list.
        assert!(add_label(&mut nbt, [4, 100, 0], "b"));
        assert_eq!(read_labels(&nbt).len(), 3);
    }

    #[test]
    fn a_label_is_removed_and_an_emptied_label_disappears_from_the_table() {
        let mut nbt = compound();
        add_label(&mut nbt, [4, 100, 0], "a");
        add_label(&mut nbt, [6, 100, 0], "a");
        assert!(remove_label(&mut nbt, [4, 100, 0], "a"));
        assert_eq!(
            read_labels(&nbt),
            vec![Labelled {
                label: "a".to_owned(),
                pos: [6, 100, 0]
            }]
        );
        assert!(remove_label(&mut nbt, [6, 100, 0], "a"));
        assert!(read_labels(&nbt).is_empty());
        // Removing what is not there changes nothing.
        assert!(!remove_label(&mut nbt, [6, 100, 0], "a"));
        let Some(Tag::Compound(table)) = tag(&nbt, LABELS) else {
            panic!("the label table key is gone, not an empty table");
        };
        assert!(table.is_empty());
    }

    #[test]
    fn a_saved_position_that_is_not_a_position_is_skipped_not_fatal() {
        let mut nbt = compound();
        let mut table = Compound::new();
        table.insert(
            "a".into(),
            Tag::List(vec![
                Tag::String("4, 100, 0".into()),
                Tag::String("nonsense".into()),
                Tag::Int(2),
            ]),
        );
        nbt.insert(LABELS.into(), Tag::Compound(table));
        assert_eq!(
            read_labels(&nbt),
            vec![Labelled {
                label: "a".to_owned(),
                pos: [4, 100, 0]
            }]
        );
    }

    #[test]
    fn the_program_hash_is_stable_and_changes_with_the_text() {
        let one = hash_text("EVERY 20 TICKS DO\nEND");
        assert_eq!(one, hash_text("EVERY 20 TICKS DO\nEND"));
        assert_ne!(one, hash_text("EVERY 40 TICKS DO\nEND"));
        assert_ne!(hash_text(""), one);
    }
}
