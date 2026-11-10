// The Factory Manager gets a block entity (API 15).
fn observe_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::sfm_state::on_block_entity(block, entity);
}
