use super::*;

fn maki_overlay(sessions: Vec<crate::maki_sessions::MakiSession>) -> ClientMakiSessionsOverlay {
    ClientMakiSessionsOverlay {
        sessions,
        now: 1_000,
        selected: 0,
        scroll: 0,
        confirm_delete: false,
        status: None,
    }
}

fn session(id: &str, cwd: &str) -> crate::maki_sessions::MakiSession {
    crate::maki_sessions::MakiSession {
        id: id.into(),
        title: format!("session {id}"),
        cwd: cwd.into(),
        model: "glm-4".into(),
        created_at: 900,
        updated_at: 990,
    }
}

fn press(state: &mut ClientShellState, code: KeyCode) -> ClientShellInput {
    let mut outcome = ClientShellInput::default();
    state.route_overlay_key(
        &crate::input::TerminalKey::new(code, KeyModifiers::empty()),
        &mut outcome,
    );
    outcome
}

#[test]
fn maki_sessions_move_and_close_with_arrow_keys_and_esc() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.overlay = Some(ClientShellOverlay::MakiSessions(maki_overlay(vec![
        session("aaa", "/repo/a"),
        session("bbb", "/repo/b"),
    ])));

    press(&mut state, KeyCode::Down);
    let Some(ClientShellOverlay::MakiSessions(overlay)) = state.overlay.as_ref() else {
        panic!("overlay should stay open");
    };
    assert_eq!(overlay.selected, 1);

    press(&mut state, KeyCode::Up);
    let Some(ClientShellOverlay::MakiSessions(overlay)) = state.overlay.as_ref() else {
        panic!("overlay should stay open");
    };
    assert_eq!(overlay.selected, 0);

    press(&mut state, KeyCode::Esc);
    assert!(state.overlay.is_none());
}

#[test]
fn maki_sessions_delete_arms_then_requires_confirmation() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.overlay = Some(ClientShellOverlay::MakiSessions(maki_overlay(vec![
        session("aaa", "/repo/a"),
    ])));

    press(&mut state, KeyCode::Char('d'));
    let Some(ClientShellOverlay::MakiSessions(overlay)) = state.overlay.as_ref() else {
        panic!("overlay should stay open");
    };
    assert!(overlay.confirm_delete);

    // Moving the selection disarms the pending delete instead of deleting.
    press(&mut state, KeyCode::Up);
    let Some(ClientShellOverlay::MakiSessions(overlay)) = state.overlay.as_ref() else {
        panic!("overlay should stay open");
    };
    assert!(!overlay.confirm_delete);
    assert_eq!(overlay.sessions.len(), 1);
}

#[test]
fn maki_sessions_enter_sends_resume_text_to_the_focused_pane() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.overlay = Some(ClientShellOverlay::MakiSessions(maki_overlay(vec![
        session("abcdefgh1234", "/repo/maki"),
    ])));

    let outcome = press(&mut state, KeyCode::Enter);
    assert!(state.overlay.is_none());
    let [action]: [ClientShellAction; 1] = outcome.actions.try_into().expect("one action");
    let ClientShellAction::Endpoint { request, .. } = action else {
        panic!("resume should issue a send-text request");
    };
    match &request.method {
        crate::api::schema::Method::PaneSendText(params) => {
            assert_eq!(params.pane_id, "pane_1");
            assert_eq!(params.text, "maki --resume abcdefgh1234\r");
        }
        other => panic!("expected pane.send_text, got {other:?}"),
    }
}

#[test]
fn maki_sessions_overlay_renders_rows_and_empty_state() {
    let state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let snapshot = Box::leak(Box::new(snapshot()));
    let render_text = |overlay| {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 106, 30));
        render::render_client_overlay(
            &mut buffer,
            &overlay,
            snapshot,
            &state.endpoints,
            &state.active_endpoint_id,
            &state.config.keybinds,
            &state.config.vim,
            &state.config.palette,
        )
        .expect("maki sessions overlay renders");
        let frame = FrameData::from_ratatui_buffer_with_hyperlinks(&buffer, None, &[]);
        frame
            .cells
            .chunks(frame.width as usize)
            .map(|row| {
                row.iter()
                    .map(|cell| cell.symbol.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let text = render_text(ClientShellOverlay::MakiSessions(maki_overlay(vec![
        session("aaaabbbb", "/repo/maki"),
    ])));
    assert!(text.contains("Maki sessions"), "{text}");
    assert!(text.contains("session aaaabbbb"), "{text}");
    assert!(text.contains("aaaabbbb"), "{text}");
    assert!(text.contains("10s"), "{text}");
    assert!(text.contains("/repo/maki"), "{text}");

    let text = render_text(ClientShellOverlay::MakiSessions(maki_overlay(Vec::new())));
    assert!(text.contains("no maki sessions found"), "{text}");
}
