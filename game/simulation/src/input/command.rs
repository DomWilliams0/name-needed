/// Command from the player through the UI
pub enum InputCommand {
    ToggleDebugRenderer { ident: &'static str, enabled: bool },
}
