use crate::app;

pub(crate) fn app_keybindings(app: &app::App) -> crate::config::LiveKeybindConfig {
    crate::config::LiveKeybindConfig {
        keybinds: app.state.keybinds.clone(),
    }
}

pub(crate) fn apply_keybindings(
    app: &mut app::App,
    keybindings: &crate::config::LiveKeybindConfig,
) {
    app.state.keybinds = keybindings.keybinds.clone();
}
