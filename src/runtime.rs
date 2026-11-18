// The pure half of the Super Factory Manager port: the SFML subset it runs and
// the item moves one program tick makes.
//
// Injected into the game as `crate::game::sfm` and built standalone as this
// crate's library, so every number and rule here is unit-tested without a
// world. Nothing in this file touches the game.
//
// Behaviour read from Super Factory Manager 4.34.0 (`ca.teamdman.sfml.ast`:
// `Program`, `TimerTrigger`, `Interval`, `InputStatement`, `OutputStatement`,
// `ResourceLimit`, `Limit`, `LabelAccess`, `Side`; `ca.teamdman.sfm.common.
// program`: `ProgramContext`, the four input/output trackers, `LimitedInputSlot`,
// `LimitedOutputSlot`; `ca.teamdman.sfm.common.blockentity.ManagerBlockEntity`).
// See `../../FINDINGS.md`.

/// Ticks in one SFML `SECOND` (`Interval` counts game ticks, 20 a second).
pub const TICKS_PER_SECOND: u64 = 20;

/// A block position in the level the program runs in.
pub type Pos = [i32; 3];

/// The sides an SFML statement may name, in `Direction::ALL`'s order
/// (`down`, `up`, `north`, `south`, `west`, `east`), which the game's
/// `Direction` indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Down,
    Up,
    North,
    South,
    West,
    East,
    /// `LEFT`/`RIGHT`/`FRONT`/`BACK`: upstream resolves these against the
    /// block's own facing; this slice has no facing to read, so they name no
    /// side and a statement that uses one moves nothing (recorded gap).
    Relative,
}

impl Side {
    /// The side's index in the game's `Direction::ALL`.
    pub fn index(self) -> Option<u8> {
        match self {
            Side::Down => Some(0),
            Side::Up => Some(1),
            Side::North => Some(2),
            Side::South => Some(3),
            Side::West => Some(4),
            Side::East => Some(5),
            Side::Relative => None,
        }
    }
}

/// One observed container slot: a position, the side it was reached through,
/// its slot number, the stack in it and whether that side may take it
/// (`native_container_slots`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub pos: Pos,
    pub side: u8,
    pub index: usize,
    pub item: String,
    pub count: u64,
    pub can_take: bool,
}

/// One label on a position, as the manager's label table holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labelled {
    pub label: String,
    pub pos: Pos,
}

/// What one program tick sees.
///
/// The glue builds it from the manager's cable network (which positions the
/// network reaches, upstream's `CableNetwork`), its label table (upstream keeps
/// it on the disk, `LabelPositionHolder`) and every labelled container's slots
/// through each side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct World {
    /// The positions the manager's network reaches: its cables and managers,
    /// and the blocks touching them. Sorted.
    pub reachable: Vec<Pos>,
    /// The manager's labels. Sorted by label then position.
    pub labels: Vec<Labelled>,
    /// The slots of the labelled, reachable containers. Sorted by position,
    /// side and slot number.
    pub slots: Vec<Slot>,
}

impl World {
    /// The reachable positions carrying `label`, in position order.
    pub fn labelled(&self, label: &str) -> Vec<Pos> {
        self.labels
            .iter()
            .filter(|entry| entry.label == label && self.reachable.contains(&entry.pos))
            .map(|entry| entry.pos)
            .collect()
    }

    /// The slots at `pos` reached through `side`.
    pub fn slots_at(&self, pos: Pos, side: u8) -> Vec<&Slot> {
        self.slots
            .iter()
            .filter(|slot| slot.pos == pos && slot.side == side)
            .collect()
    }

    /// The sides of `pos` whose list of slots is not empty, in `Side::ALL`
    /// order: a block reached from several sides (a chest) is one container.
    pub fn sides_with_slots(&self, pos: Pos) -> Vec<u8> {
        (0..6)
            .filter(|side| {
                self.slots
                    .iter()
                    .any(|slot| slot.pos == pos && slot.side == *side)
            })
            .collect()
    }
}

/// One item move a program tick makes: how much of `item` may go from one
/// container side to another. The glue runs it through
/// `native_move_items`, which moves item by item and keeps what the target
/// refuses, so a move never loses an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub from: Pos,
    pub from_side: u8,
    pub to: Pos,
    pub to_side: u8,
    pub item: String,
    pub max: u64,
}

/// A resource id pattern: `` * `` matches any run of characters, a bare id
/// without a namespace is `minecraft:`'s, and anything else is exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern(String);

impl Pattern {
    /// Whether `id` (`minecraft:iron_ingot`) matches.
    ///
    /// A pattern with `*` is a glob, one with a namespace is exact, and a bare
    /// id names `minecraft:`'s resource (upstream's `ResourceIdentifier`
    /// default namespace).
    pub fn matches(&self, id: &str) -> bool {
        let pattern = self.0.as_str();
        if pattern.contains('*') {
            return glob(pattern, id);
        }
        if pattern.contains(':') {
            return pattern == id;
        }
        id == pattern || id.strip_prefix("minecraft:") == Some(pattern)
    }
}

/// `*` matches any run of characters; everything else is literal.
fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let mut rest = text;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 {
            let Some(after) = rest.strip_prefix(part) else {
                return false;
            };
            rest = after;
        } else if index == parts.len() - 1 {
            return rest.ends_with(part);
        } else {
            let Some(at) = rest.find(part) else {
                return false;
            };
            rest = &rest[at + part.len()..];
        }
    }
    true
}

/// What an `INPUT`/`OUTPUT` statement takes or gives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// The ids it accepts; empty means every id.
    pub ids: Vec<Pattern>,
    /// The ids it leaves alone (`EXCEPT`).
    pub except: Vec<Pattern>,
}

impl Filter {
    /// Whether `id` is one this statement handles.
    pub fn matches(&self, id: &str) -> bool {
        let accepted = self.ids.is_empty() || self.ids.iter().any(|pattern| pattern.matches(id));
        accepted && !self.except.iter().any(|pattern| pattern.matches(id))
    }
}

/// A quantity or retention budget: one total shared by every resource id, or
/// one per matching id (`EACH`), or none at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Budget {
    Unlimited,
    Shared(u64),
    Each(u64),
}

impl Budget {
    /// What is left of this budget for `item` after `spent`.
    fn left(self, item: &str, spent: &Spent) -> u64 {
        match self {
            Budget::Unlimited => u64::MAX,
            Budget::Shared(total) => total.saturating_sub(spent.total),
            Budget::Each(total) => total.saturating_sub(spent.of(item)),
        }
    }
}

/// What a budget has been spent on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Spent {
    total: u64,
    by_item: Vec<(String, u64)>,
}

impl Spent {
    fn of(&self, item: &str) -> u64 {
        self.by_item
            .iter()
            .find(|(id, _)| id == item)
            .map_or(0, |(_, amount)| *amount)
    }

    fn add(&mut self, item: &str, amount: u64) {
        self.total = self.total.saturating_add(amount);
        match self.by_item.iter_mut().find(|(id, _)| id == item) {
            Some((_, total)) => *total = total.saturating_add(amount),
            None => self.by_item.push((item.to_owned(), amount)),
        }
    }
}

/// An `INPUT` or `OUTPUT` statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Io {
    pub quantity: Budget,
    pub retention: Budget,
    pub filter: Filter,
    /// The labels the statement names.
    pub labels: Vec<String>,
    /// The sides it names; empty means every side.
    pub sides: Vec<Side>,
    /// `EACH` after `FROM`/`TO`: a separate tracker per block. Parsed and
    /// recorded, but inert in this slice (recorded gap).
    pub each_block: bool,
}

/// One statement of a trigger's block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Input(Io),
    Output(Io),
    /// `FORGET`: drop what the inputs gathered, so a later `OUTPUT` moves
    /// nothing of it.
    Forget,
}

/// `EVERY <interval> DO … END`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// The interval in ticks (`EVERY 300 SECONDS` is 6,000).
    pub interval: u64,
    pub statements: Vec<Statement>,
}

/// A parsed program: its name and its triggers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub name: Option<String>,
    pub triggers: Vec<Trigger>,
}

impl Program {
    /// Parses SFML.
    ///
    /// # Errors
    ///
    /// A line this slice cannot read: an unknown keyword, a trigger that never
    /// ends, a statement outside a trigger, a number that is not one.
    pub fn parse(text: &str) -> Result<Program, ParseError> {
        let mut program = Program::default();
        let mut open: Option<Trigger> = None;
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let tokens = tokenise(raw).map_err(|message| ParseError { line, message })?;
            let Some(first) = tokens.first() else {
                continue;
            };
            let keyword = first.to_ascii_uppercase();
            let error = |message: String| ParseError { line, message };
            match open.as_mut() {
                None => match keyword.as_str() {
                    "NAME" => {
                        let name = tokens
                            .get(1)
                            .ok_or_else(|| error("NAME needs a name".into()))?;
                        program.name = Some(name.clone());
                    }
                    "EVERY" => open = Some(parse_every(&tokens).map_err(error)?),
                    _ => {
                        return Err(error(format!(
                            "{keyword} outside EVERY … DO … END, which this slice needs first"
                        )));
                    }
                },
                Some(trigger) => match keyword.as_str() {
                    "END" => {
                        program
                            .triggers
                            .push(open.take().expect("a trigger is open"));
                    }
                    "FORGET" => trigger.statements.push(Statement::Forget),
                    "INPUT" => trigger
                        .statements
                        .push(Statement::Input(parse_io(&tokens).map_err(error)?)),
                    "OUTPUT" => trigger
                        .statements
                        .push(Statement::Output(parse_io(&tokens).map_err(error)?)),
                    _ => {
                        return Err(error(format!(
                            "{keyword} is not a statement this slice runs"
                        )));
                    }
                },
            }
        }
        if open.is_some() {
            return Err(ParseError {
                line: text.lines().count().max(1),
                message: "EVERY … DO has no END".into(),
            });
        }
        Ok(program)
    }

    /// Whether the program can do nothing: no trigger, or triggers with no
    /// statement.
    pub fn is_empty(&self) -> bool {
        self.triggers
            .iter()
            .all(|trigger| trigger.statements.is_empty())
    }
}

/// A program this slice cannot read, with the line it stopped at (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Whether a trigger is due on this manager tick: upstream's
/// `Interval.shouldTick` with `LOCAL` alignment and offset 0, i.e.
/// `manager.tick % interval == 0`.
pub fn due(trigger: &Trigger, manager_tick: u64) -> bool {
    trigger.interval > 0 && manager_tick.is_multiple_of(trigger.interval)
}

/// Splits one line into tokens: whitespace and commas separate, `"…"` keeps a
/// token whole (a label with spaces), and `--` starts a comment.
fn tokenise(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    fn flush(current: &mut String, tokens: &mut Vec<String>) {
        if !current.is_empty() {
            tokens.push(std::mem::take(current));
        }
    }
    while let Some(character) = chars.next() {
        match character {
            '"' => quoted = !quoted,
            '-' if !quoted && chars.peek() == Some(&'-') => break,
            ',' | ' ' | '\t' if !quoted => flush(&mut current, &mut tokens),
            other => current.push(other),
        }
    }
    if quoted {
        return Err("a quoted string is never closed".into());
    }
    flush(&mut current, &mut tokens);
    Ok(tokens)
}

/// `EVERY <interval> (TICKS|SECONDS) DO`.
fn parse_every(tokens: &[String]) -> Result<Trigger, String> {
    let word = tokens
        .get(1)
        .ok_or_else(|| "EVERY needs an interval".to_owned())?;
    let interval = parse_number(word).ok_or_else(|| format!("{word} is not a number"))?;
    if interval == 0 {
        return Err("an interval of 0 ticks never fires".into());
    }
    let unit = tokens
        .get(2)
        .ok_or_else(|| "EVERY needs TICKS or SECONDS".to_owned())?
        .to_ascii_uppercase();
    let interval = match unit.as_str() {
        "TICKS" => interval,
        "SECONDS" => interval.saturating_mul(TICKS_PER_SECOND),
        other => return Err(format!("{other} is not TICKS or SECONDS")),
    };
    let do_word = tokens
        .get(3)
        .ok_or_else(|| "EVERY … TICKS needs DO".to_owned())?
        .to_ascii_uppercase();
    if do_word != "DO" {
        return Err(format!("{do_word} where DO belongs"));
    }
    Ok(Trigger {
        interval,
        statements: Vec::new(),
    })
}

fn parse_number(word: &str) -> Option<u64> {
    word.parse::<u64>().ok()
}

fn is_keyword(token: &str, keyword: &str) -> bool {
    token.eq_ignore_ascii_case(keyword)
}

/// `INPUT … FROM …` or `OUTPUT … TO …`, as `ASTBuilder` reads it.
fn parse_io(tokens: &[String]) -> Result<Io, String> {
    let heading = tokens[0].to_ascii_uppercase();
    let input = heading == "INPUT";
    let mut index = 1;

    let mut quantity = Budget::Unlimited;
    if let Some(amount) = tokens.get(index).and_then(|word| parse_number(word)) {
        quantity = Budget::Shared(amount);
        index += 1;
        if tokens
            .get(index)
            .is_some_and(|token| is_keyword(token, "EACH"))
        {
            quantity = Budget::Each(amount);
            index += 1;
        }
    }

    let mut retention = if input {
        Budget::Shared(0)
    } else {
        Budget::Unlimited
    };
    if tokens
        .get(index)
        .is_some_and(|token| is_keyword(token, "RETAIN"))
    {
        index += 1;
        let word = tokens
            .get(index)
            .ok_or_else(|| "RETAIN needs a number".to_owned())?;
        let amount = parse_number(word).ok_or_else(|| format!("{word} is not a number"))?;
        index += 1;
        retention = Budget::Shared(amount);
        if tokens
            .get(index)
            .is_some_and(|token| is_keyword(token, "EACH"))
        {
            retention = Budget::Each(amount);
            index += 1;
        }
    }

    let mut ids = Vec::new();
    while let Some(token) = tokens.get(index) {
        if is_keyword(token, "FROM") || is_keyword(token, "TO") || is_keyword(token, "EXCEPT") {
            break;
        }
        ids.push(Pattern(token.clone()));
        index += 1;
    }
    let mut except = Vec::new();
    if tokens
        .get(index)
        .is_some_and(|token| is_keyword(token, "EXCEPT"))
    {
        index += 1;
        while let Some(token) = tokens.get(index) {
            if is_keyword(token, "FROM") || is_keyword(token, "TO") {
                break;
            }
            except.push(Pattern(token.clone()));
            index += 1;
        }
    }

    let joint = if input { "FROM" } else { "TO" };
    match tokens.get(index) {
        Some(token) if is_keyword(token, joint) => index += 1,
        _ => return Err(format!("{heading} needs {joint} <labels>")),
    }

    let mut each_block = false;
    if tokens
        .get(index)
        .is_some_and(|token| is_keyword(token, "EACH"))
    {
        each_block = true;
        index += 1;
    }

    // A trailing `… SIDE` names the sides: the run of side words right before
    // it belongs to the statement, everything earlier is a label.
    let rest = &tokens[index..];
    let mut sides = Vec::new();
    let mut label_words = rest;
    if let Some(side_at) = rest.iter().rposition(|token| is_keyword(token, "SIDE")) {
        let mut start = side_at;
        while start > 0 && parse_side(&rest[start - 1]).is_ok() {
            start -= 1;
        }
        for word in &rest[start..side_at] {
            sides.push(parse_side(word)?);
        }
        label_words = &rest[..start];
    }
    for word in label_words {
        if is_keyword(word, "EMPTY") || is_keyword(word, "SLOTS") || is_keyword(word, "IN") {
            return Err(format!(
                "EMPTY SLOTS IN is not in this slice ({word} follows {joint})"
            ));
        }
    }
    let labels: Vec<String> = label_words.to_vec();
    if labels.is_empty() {
        return Err(format!("{heading} names no label"));
    }

    Ok(Io {
        quantity,
        retention,
        filter: Filter { ids, except },
        labels,
        sides,
        each_block,
    })
}

fn parse_side(word: &str) -> Result<Side, String> {
    match word.to_ascii_uppercase().as_str() {
        "TOP" => Ok(Side::Up),
        "BOTTOM" => Ok(Side::Down),
        "NORTH" => Ok(Side::North),
        "SOUTH" => Ok(Side::South),
        "WEST" => Ok(Side::West),
        "EAST" => Ok(Side::East),
        "LEFT" | "RIGHT" | "FRONT" | "BACK" => Ok(Side::Relative),
        other => Err(format!("{other} is not a side")),
    }
}

/// One slot an `INPUT` statement gathered, and how much of it may still move.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    pos: Pos,
    side: u8,
    item: String,
    remaining: u64,
}

/// The sides of `pos` a statement reads, one per distinct set of slots: a
/// chest reports the same 27 slots through every face (one side), a furnace
/// reports input, fuel and output through different ones (three).
fn resolve_sides(io: &Io, world: &World, pos: Pos) -> Vec<u8> {
    let candidates: Vec<u8> = if io.sides.is_empty() {
        world.sides_with_slots(pos)
    } else {
        io.sides
            .iter()
            .filter_map(|side| side.index())
            .filter(|side| !world.slots_at(pos, *side).is_empty())
            .collect()
    };
    let mut kept: Vec<(Vec<usize>, u8)> = Vec::new();
    for side in candidates {
        let indices: Vec<usize> = world
            .slots_at(pos, side)
            .iter()
            .map(|slot| slot.index)
            .collect();
        if kept.iter().any(|(seen, _)| *seen == indices) {
            continue;
        }
        kept.push((indices, side));
    }
    kept.into_iter().map(|(_, side)| side).collect()
}

/// What `INPUT` statements have gathered so far in this tick.
fn gather(io: &Io, world: &World, pending: &mut Vec<Pending>) {
    let mut quantity_spent = Spent::default();
    let mut retention_spent = Spent::default();
    for label in &io.labels {
        for pos in world.labelled(label) {
            for side in resolve_sides(io, world, pos) {
                for slot in world.slots_at(pos, side) {
                    if slot.count == 0 || !slot.can_take || !io.filter.matches(&slot.item) {
                        continue;
                    }
                    let item = slot.item.as_str();
                    // Upstream dedicates up to `retention` of each slot to what
                    // stays behind, then caps the move by the shared quantity.
                    let obligation = slot.count.min(io.retention.left(item, &retention_spent));
                    retention_spent.add(item, obligation);
                    let movable = slot.count.saturating_sub(obligation);
                    let take = movable.min(io.quantity.left(item, &quantity_spent));
                    if take == 0 {
                        continue;
                    }
                    quantity_spent.add(item, take);
                    pending.push(Pending {
                        pos,
                        side,
                        item: slot.item.clone(),
                        remaining: take,
                    });
                }
            }
        }
    }
}

/// `OUTPUT`: what the inputs gathered goes into the output labels' slots, in
/// order, capped by the statement's quantity and retention.
fn output(io: &Io, world: &World, pending: &mut [Pending], moves: &mut Vec<Move>) {
    if pending.is_empty() {
        return;
    }
    let mut targets: Vec<(Pos, u8)> = Vec::new();
    for label in &io.labels {
        for pos in world.labelled(label) {
            for side in resolve_sides(io, world, pos) {
                if targets.contains(&(pos, side)) {
                    continue;
                }
                targets.push((pos, side));
            }
        }
    }
    if targets.is_empty() {
        return;
    }
    // The output retention is measured against what the target already holds
    // (`updateRetentionObservation`) plus what has gone in this tick.
    let mut retention_spent = Spent::default();
    for (pos, side) in &targets {
        for slot in world.slots_at(*pos, *side) {
            if io.filter.matches(&slot.item) {
                retention_spent.add(&slot.item, slot.count);
            }
        }
    }
    let mut quantity_spent = Spent::default();
    for entry in pending.iter_mut() {
        if entry.remaining == 0 || !io.filter.matches(&entry.item) {
            continue;
        }
        for (pos, side) in &targets {
            if entry.remaining == 0 {
                break;
            }
            let item = entry.item.as_str();
            let room = io
                .retention
                .left(item, &retention_spent)
                .min(io.quantity.left(item, &quantity_spent))
                .min(entry.remaining);
            if room == 0 {
                continue;
            }
            entry.remaining -= room;
            quantity_spent.add(item, room);
            retention_spent.add(item, room);
            moves.push(Move {
                from: entry.pos,
                from_side: entry.side,
                to: *pos,
                to_side: *side,
                item: entry.item.clone(),
                max: room,
            });
        }
    }
}

/// The moves one trigger makes on one tick, in program order.
///
/// `INPUT` gathers the slots its labels reach and `OUTPUT` moves what they
/// gathered, as upstream's `OutputStatement.tick` walks its input slots into
/// its output slots. Nothing here changes the world: the glue runs the moves.
pub fn plan(trigger: &Trigger, world: &World) -> Vec<Move> {
    let mut moves = Vec::new();
    let mut pending: Vec<Pending> = Vec::new();
    for statement in &trigger.statements {
        match statement {
            Statement::Input(io) => gather(io, world, &mut pending),
            Statement::Output(io) => output(io, world, &mut pending, &mut moves),
            Statement::Forget => pending.clear(),
        }
    }
    moves
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i32) -> Pos {
        [x, 0, 0]
    }

    fn program(source: &str) -> Program {
        Program::parse(source).expect("the program parses")
    }

    fn trigger(source: &str) -> Trigger {
        program(source).triggers.remove(0)
    }

    /// A world of one label per position, every position reachable.
    fn scene(slots: &[(Pos, u8, &str, u64)], labels: &[(&str, Pos)]) -> World {
        World {
            reachable: labels.iter().map(|(_, pos)| *pos).collect(),
            labels: labels
                .iter()
                .map(|(label, pos)| Labelled {
                    label: (*label).to_owned(),
                    pos: *pos,
                })
                .collect(),
            slots: slots
                .iter()
                .map(|(pos, side, item, count)| Slot {
                    pos: *pos,
                    side: *side,
                    index: 0,
                    item: (*item).to_owned(),
                    count: *count,
                    can_take: true,
                })
                .collect(),
        }
    }

    // ---------------------------------------------------------------------
    // The language
    // ---------------------------------------------------------------------

    #[test]
    fn a_name_and_one_timer_trigger_parse() {
        let parsed = program(
            "NAME \"A simple program\"\n\nEVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT TO b\nEND\n",
        );
        assert_eq!(parsed.name.as_deref(), Some("A simple program"));
        assert_eq!(parsed.triggers.len(), 1);
        assert_eq!(parsed.triggers[0].interval, 20);
        assert_eq!(parsed.triggers[0].statements.len(), 2);
    }

    #[test]
    fn keywords_are_case_insensitive_and_seconds_are_ticks() {
        let parsed = program("every 300 seconds do\n  forget\nend");
        assert_eq!(parsed.triggers[0].interval, 6000);
        assert_eq!(parsed.triggers[0].statements, vec![Statement::Forget]);
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let parsed = program(
            "-- a header\n\nEVERY 20 TICKS DO\n    -- move it\n    INPUT FROM a -- trailing\nEND\n",
        );
        assert_eq!(parsed.triggers[0].statements.len(), 1);
    }

    #[test]
    fn an_input_reads_its_quantity_retention_filter_and_sides() {
        let parsed = trigger("EVERY 20 TICKS DO\n    INPUT 5 RETAIN 3 stone FROM a\nEND");
        let Statement::Input(io) = &parsed.statements[0] else {
            panic!("expected an input");
        };
        assert_eq!(io.quantity, Budget::Shared(5));
        assert_eq!(io.retention, Budget::Shared(3));
        assert!(io.filter.matches("minecraft:stone"));
        assert!(!io.filter.matches("minecraft:dirt"));
        assert_eq!(io.labels, vec!["a".to_owned()]);
        assert!(io.sides.is_empty());
    }

    #[test]
    fn each_expands_the_quantity_and_the_retention_per_resource() {
        let parsed =
            trigger("EVERY 20 TICKS DO\n    INPUT 5 EACH RETAIN 3 EACH *ingot* FROM a\nEND");
        let Statement::Input(io) = &parsed.statements[0] else {
            panic!("expected an input");
        };
        assert_eq!(io.quantity, Budget::Each(5));
        assert_eq!(io.retention, Budget::Each(3));
        assert!(io.filter.matches("minecraft:iron_ingot"));
        assert!(!io.filter.matches("minecraft:stone"));
    }

    #[test]
    fn a_statement_without_a_limit_has_upstreams_defaults() {
        let parsed = trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT TO b\nEND");
        let Statement::Input(input) = &parsed.statements[0] else {
            panic!("expected an input");
        };
        let Statement::Output(output) = &parsed.statements[1] else {
            panic!("expected an output");
        };
        // `TAKE_ALL_LEAVE_NONE` and `ACCEPT_ALL_WITHOUT_RESTRAINT`.
        assert_eq!(input.quantity, Budget::Unlimited);
        assert_eq!(input.retention, Budget::Shared(0));
        assert_eq!(output.quantity, Budget::Unlimited);
        assert_eq!(output.retention, Budget::Unlimited);
    }

    #[test]
    fn labels_can_be_quoted_or_a_comma_separated_list() {
        let parsed =
            trigger("EVERY 20 TICKS DO\n    INPUT FROM \"to be smelted\"\n    OUTPUT TO a, b\nEND");
        let Statement::Input(input) = &parsed.statements[0] else {
            panic!("expected an input");
        };
        let Statement::Output(output) = &parsed.statements[1] else {
            panic!("expected an output");
        };
        assert_eq!(input.labels, vec!["to be smelted".to_owned()]);
        assert_eq!(output.labels, vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn a_side_qualifier_is_read_after_the_label() {
        let parsed =
            trigger("EVERY 20 TICKS DO\n    OUTPUT RETAIN 1 coal TO furnace BOTTOM SIDE\nEND");
        let Statement::Output(io) = &parsed.statements[0] else {
            panic!("expected an output");
        };
        assert_eq!(io.labels, vec!["furnace".to_owned()]);
        assert_eq!(io.sides, vec![Side::Down]);
    }

    #[test]
    fn several_sides_and_the_each_keyword_after_from_parse() {
        let parsed = trigger(
            "EVERY 20 TICKS DO\n    INPUT 1 \"redstone\" FROM EACH a TOP, BOTTOM SIDE\nEND",
        );
        let Statement::Input(io) = &parsed.statements[0] else {
            panic!("expected an input");
        };
        assert!(io.each_block);
        assert_eq!(io.sides, vec![Side::Up, Side::Down]);
        assert_eq!(io.filter.ids, vec![Pattern("redstone".to_owned())]);
    }

    #[test]
    fn except_and_a_trailing_comma_are_read() {
        let parsed = trigger(
            "EVERY 20 TICKS DO\n    INPUT *ingot* EXCEPT iron_ingot, gold_ingot, FROM a\n    OUTPUT 5 EXCEPT copper_ingot TO b\nEND",
        );
        let Statement::Input(input) = &parsed.statements[0] else {
            panic!("expected an input");
        };
        assert!(input.filter.matches("minecraft:iron_ingot_2"));
        assert!(!input.filter.matches("minecraft:iron_ingot"));
        let Statement::Output(output) = &parsed.statements[1] else {
            panic!("expected an output");
        };
        assert!(!output.filter.matches("minecraft:copper_ingot"));
        assert!(output.filter.matches("minecraft:gold_ingot"));
    }

    #[test]
    fn a_line_this_slice_cannot_read_names_its_line() {
        let error = Program::parse("EVERY 20 TICKS DO\n    OUTPUT TO EMPTY SLOTS IN b\nEND")
            .expect_err("an unsupported keyword is an error");
        assert_eq!(error.line, 2);
        assert!(error.message.contains("EMPTY"), "{}", error.message);
    }

    #[test]
    fn an_unclosed_trigger_is_an_error() {
        let error = Program::parse("EVERY 20 TICKS DO\n    INPUT FROM a\n")
            .expect_err("a trigger without END is an error");
        assert!(error.message.contains("END"), "{}", error.message);
    }

    #[test]
    fn a_statement_outside_a_trigger_is_an_error() {
        let error = Program::parse("INPUT FROM a").expect_err("no trigger to hold it");
        assert!(error.message.contains("EVERY"), "{}", error.message);
    }

    // ---------------------------------------------------------------------
    // The tick
    // ---------------------------------------------------------------------

    #[test]
    fn a_timer_fires_on_the_manager_tick_its_interval_divides() {
        let every_twenty = trigger("EVERY 20 TICKS DO\n    FORGET\nEND");
        // Upstream's `Interval.shouldTick` is `tick % ticks == offset`; the
        // manager counts up before it ticks its program, so 0 is never seen.
        assert!(due(&every_twenty, 0));
        assert!(!due(&every_twenty, 1));
        assert!(!due(&every_twenty, 19));
        assert!(due(&every_twenty, 20));
        assert!(!due(&every_twenty, 21));
        assert!(due(&every_twenty, 40));
    }

    // ---------------------------------------------------------------------
    // The moves
    // ---------------------------------------------------------------------

    #[test]
    fn a_whole_stack_moves_from_the_input_label_to_the_output_label() {
        let tick = trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT TO b\nEND");
        let world = scene(
            &[
                (at(0), 1, "minecraft:diamond", 5),
                (at(1), 1, "minecraft:dirt", 1),
            ],
            &[("a", at(0)), ("b", at(1))],
        );
        assert_eq!(
            plan(&tick, &world),
            vec![Move {
                from: at(0),
                from_side: 1,
                to: at(1),
                to_side: 1,
                item: "minecraft:diamond".to_owned(),
                max: 5,
            }]
        );
    }

    #[test]
    fn a_quantity_is_shared_by_every_slot_it_gathers() {
        let tick = trigger("EVERY 20 TICKS DO\n    INPUT 5 FROM a\n    OUTPUT TO b\nEND");
        let mut observed = scene(
            &[
                (at(0), 1, "minecraft:stone", 3),
                (at(2), 1, "minecraft:stone", 4),
                (at(1), 1, "", 0),
            ],
            &[("a", at(0)), ("b", at(1))],
        );
        observed.labels.push(Labelled {
            label: "a".to_owned(),
            pos: at(2),
        });
        observed.reachable.push(at(2));
        let moves = plan(&tick, &observed);
        assert_eq!(moves.len(), 2);
        assert_eq!((moves[0].from, moves[0].max), (at(0), 3));
        assert_eq!((moves[1].from, moves[1].max), (at(2), 2));
    }

    #[test]
    fn each_gives_every_matching_resource_its_own_quantity() {
        let tick =
            trigger("EVERY 20 TICKS DO\n    INPUT 5 EACH *ingot* FROM a\n    OUTPUT TO b\nEND");
        let world = scene(
            &[
                (at(0), 1, "minecraft:iron_ingot", 8),
                (at(0), 1, "minecraft:gold_ingot", 2),
                (at(1), 1, "", 0),
            ],
            &[("a", at(0)), ("b", at(1))],
        );
        let moves = plan(&tick, &world);
        assert_eq!(moves.len(), 2);
        assert_eq!(
            (moves[0].item.as_str(), moves[0].max),
            ("minecraft:iron_ingot", 5)
        );
        assert_eq!(
            (moves[1].item.as_str(), moves[1].max),
            ("minecraft:gold_ingot", 2)
        );
    }

    #[test]
    fn a_retention_leaves_that_many_in_the_source() {
        let tick = trigger("EVERY 20 TICKS DO\n    INPUT 10 RETAIN 3 FROM a\n    OUTPUT TO b\nEND");
        let world = scene(
            &[(at(0), 1, "minecraft:stone", 10), (at(1), 1, "", 0)],
            &[("a", at(0)), ("b", at(1))],
        );
        let moves = plan(&tick, &world);
        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].max, 7);
    }

    #[test]
    fn a_retention_larger_than_the_stack_moves_nothing() {
        let tick = trigger("EVERY 20 TICKS DO\n    INPUT RETAIN 3 FROM a\n    OUTPUT TO b\nEND");
        let world = scene(
            &[(at(0), 1, "minecraft:stone", 2), (at(1), 1, "", 0)],
            &[("a", at(0)), ("b", at(1))],
        );
        assert!(plan(&tick, &world).is_empty());
    }

    #[test]
    fn a_filter_only_moves_what_it_names() {
        let tick = trigger("EVERY 20 TICKS DO\n    INPUT stone FROM a\n    OUTPUT TO b\nEND");
        let world = scene(
            &[
                (at(0), 1, "minecraft:stone", 3),
                (at(0), 1, "minecraft:dirt", 3),
                (at(1), 1, "", 0),
            ],
            &[("a", at(0)), ("b", at(1))],
        );
        let moves = plan(&tick, &world);
        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].item, "minecraft:stone");
    }

    #[test]
    fn an_output_quantity_and_retention_cap_what_goes_in() {
        let capped = trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT 4 TO b\nEND");
        let world = scene(
            &[
                (at(0), 1, "minecraft:stone", 9),
                (at(1), 1, "minecraft:dirt", 1),
                (at(1), 1, "", 0),
            ],
            &[("a", at(0)), ("b", at(1))],
        );
        assert_eq!(plan(&capped, &world)[0].max, 4);

        let topped_up =
            trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT RETAIN 5 TO b\nEND");
        let world = scene(
            &[
                (at(0), 1, "minecraft:stone", 9),
                (at(1), 1, "minecraft:stone", 3),
            ],
            &[("a", at(0)), ("b", at(1))],
        );
        assert_eq!(plan(&topped_up, &world)[0].max, 2);

        let already_full =
            trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT RETAIN 5 TO b\nEND");
        let world = scene(
            &[
                (at(0), 1, "minecraft:stone", 9),
                (at(1), 1, "minecraft:stone", 5),
            ],
            &[("a", at(0)), ("b", at(1))],
        );
        assert!(plan(&already_full, &world).is_empty());
    }

    #[test]
    fn forget_drops_what_the_inputs_gathered() {
        let tick = trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    FORGET\n    OUTPUT TO b\nEND");
        let world = scene(
            &[(at(0), 1, "minecraft:stone", 9), (at(1), 1, "", 0)],
            &[("a", at(0)), ("b", at(1))],
        );
        assert!(plan(&tick, &world).is_empty());
    }

    #[test]
    fn a_second_output_takes_what_the_first_left() {
        let tick =
            trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT 4 TO b\n    OUTPUT TO c\nEND");
        let world = scene(
            &[
                (at(0), 1, "minecraft:stone", 9),
                (at(1), 1, "", 0),
                (at(3), 1, "", 0),
            ],
            &[("a", at(0)), ("b", at(1)), ("c", at(3))],
        );
        let moves = plan(&tick, &world);
        assert_eq!(moves.len(), 2);
        assert_eq!((moves[0].to, moves[0].max), (at(1), 4));
        assert_eq!((moves[1].to, moves[1].max), (at(3), 5));
    }

    #[test]
    fn a_label_off_the_network_moves_nothing() {
        let tick = trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT TO b\nEND");
        let mut world = scene(
            &[(at(0), 1, "minecraft:stone", 9), (at(1), 1, "", 0)],
            &[("a", at(0)), ("b", at(1))],
        );
        // The source is labelled but no cable reaches it.
        world.reachable.retain(|pos| *pos != at(0));
        assert!(plan(&tick, &world).is_empty());
    }

    #[test]
    fn a_side_qualifier_picks_the_face_and_an_unqualified_label_the_first_with_slots() {
        let named =
            trigger("EVERY 20 TICKS DO\n    INPUT FROM a BOTTOM SIDE\n    OUTPUT TO b\nEND");
        let world = scene(
            &[
                (at(0), 0, "minecraft:stone", 4),
                (at(0), 1, "minecraft:dirt", 7),
                (at(1), 1, "", 0),
            ],
            &[("a", at(0)), ("b", at(1))],
        );
        let moves = plan(&named, &world);
        assert_eq!(
            (moves[0].from_side, moves[0].item.as_str()),
            (0, "minecraft:stone")
        );

        let unqualified = trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT TO b\nEND");
        let moves = plan(&unqualified, &world);
        assert_eq!(
            (moves[0].from_side, moves[0].item.as_str()),
            (0, "minecraft:stone")
        );
    }

    #[test]
    fn a_slot_the_side_cannot_take_is_left_alone() {
        let tick = trigger("EVERY 20 TICKS DO\n    INPUT FROM a\n    OUTPUT TO b\nEND");
        let mut world = scene(
            &[(at(0), 1, "minecraft:stone", 4), (at(1), 1, "", 0)],
            &[("a", at(0)), ("b", at(1))],
        );
        world.slots[0].can_take = false;
        assert!(plan(&tick, &world).is_empty());
    }

    #[test]
    fn a_program_with_no_statement_makes_no_move() {
        let empty = program("NAME \"nothing\"");
        assert!(empty.is_empty());
        let tick = trigger("EVERY 20 TICKS DO\nEND");
        assert!(plan(&tick, &World::default()).is_empty());
    }
}
