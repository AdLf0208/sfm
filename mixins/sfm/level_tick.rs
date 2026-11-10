// After the level's tick: count each manager up and run the programs whose
// timer is due (API 23).
fn observe_level_tick_post(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::sfm_state::on_level_tick_post(game, dimension);
}
