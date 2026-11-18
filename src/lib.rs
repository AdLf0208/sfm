//! Super Factory Manager for Horizon — slice 1: the Factory Manager, its cable network and
//! the compiled SFML subset it runs.
//!
//! Ported from Super Factory Manager 4.34.0 (TeamDman/SuperFactoryManager, MPL-2.0); see
//! `FINDINGS.md` for the behaviour read from the jar and the upstream repository, and
//! `notes.md` for the hook map, the gaps and the migration to the SDK format.
//!
//! The manager is a **block entity**, so its moments are a Part attached to the type this mod
//! declares: its tick queues the manager, and the program's moves run once at
//! `level_tick_post` (the SDK cannot enumerate a level's block entities, and a block entity
//! is out of the world during its own tick). The rules — the parser, the timer predicate,
//! the plan a trigger makes and the retention accounting — live in [`runtime`], which has no
//! `mod_api` type and is unit-tested by `cargo test` alone. [`support`] is the game glue: the
//! manager's saved compound, the cable network, the container reads, the moves and `/sfm`.

pub mod runtime;
mod support;

use mod_api::{
    self, Flow, Hooks, HostSide, LevelTickPostEvent, RegisterCommandsEvent, Registrar,
    RegistrationContext, RunModCommandEvent, ServerHost,
};

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare the manager's block entity type and its Part.
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        support::register(registrar);
    }

    /// `/sfm program|show|label|unlabel|labels|tick` (upstream has no commands; the program
    /// editor is a GUI on the disk, which this slice does not port).
    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        support::declare_commands(event);
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        support::run_command(host, event)
    }

    /// After the level's tick: every manager that ticked this tick counts up and runs the
    /// programs whose timer is due. A client's level has no server block entities to run.
    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        if let HostSide::Server(server) = host {
            support::on_level_tick_post(server, event.level);
        }
    }
}
