// `/sfm …` runs here, with the player's state swapped in (API 8).
fn observe_command(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::sfm_state::on_perform_command(game, player, command, ran);
}
