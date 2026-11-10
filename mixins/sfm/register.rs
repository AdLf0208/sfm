// `/sfm …` is registered at the command hook's head (API 8).
fn observe_register(commands: &mut Vec<NativeCommand>) {
    crate::game::sfm_state::on_register_commands(commands);
}
