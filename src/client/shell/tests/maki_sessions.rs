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

fn pane_with_cwd(pane_id: &str, cwd: &str) -> ClientShellPane {
    ClientShellPane {
        pane_id: pane_id.into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        label: None,
        cwd: Some(cwd.into()),
        foreground_cwd: Some(cwd.into()),
        focused: false,
        right_click_passthrough: false,
    }
}

fn process_info_response(
    pane_id: &str,
    shell_idle: bool,
    foreground_processes: Vec<crate::api::schema::PaneProcessInfoProcess>,
) -> crate::api::schema::ResponseResult {
    crate::api::schema::ResponseResult::PaneProcessInfo {
        process_info: crate::api::schema::PaneProcessInfo {
            pane_id: pane_id.into(),
            shell_pid: None,
            foreground_process_group_id: None,
            tty: None,
            shell_idle: Some(shell_idle),
            foreground_processes,
        },
    }
}

fn endpoint_request(action: ClientShellAction) -> crate::api::schema::Request {
    let ClientShellAction::Endpoint { request, .. } = action else {
        panic!("expected endpoint action");
    };
    *request
}

fn busy_process() -> crate::api::schema::PaneProcessInfoProcess {
    crate::api::schema::PaneProcessInfoProcess {
        pid: 42,
        name: "vim".into(),
        argv0: None,
        argv: None,
        cmdline: None,
        cwd: None,
    }
}

#[test]
fn maki_sessions_enter_probes_matching_cwd_and_resumes_there() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut snap = snapshot();
    snap.panes.push(pane_with_cwd("pane_2", "/repo/maki"));
    state.set_snapshot(Box::new(snap));
    state.set_pane_surface(surface());
    state.overlay = Some(ClientShellOverlay::MakiSessions(maki_overlay(vec![
        session("abcdefgh1234", "/repo/maki"),
    ])));

    let outcome = press(&mut state, KeyCode::Enter);
    assert!(state.overlay.is_none());
    let [action]: [ClientShellAction; 1] = outcome.actions.try_into().expect("one action");
    let probe = endpoint_request(action);
    match &probe.method {
        crate::api::schema::Method::PaneProcessInfo(params) => {
            assert_eq!(params.pane_id.as_deref(), Some("pane_2"));
        }
        other => panic!("expected pane.process_info, got {other:?}"),
    }

    let (repaint, actions) = state.handle_endpoint_result(
        "boot-1",
        &probe.id,
        Ok(process_info_response("pane_2", true, Vec::new())),
    );
    assert!(repaint);
    let [focus, send]: [ClientShellAction; 2] = actions.try_into().expect("focus then send");
    let focus = endpoint_request(focus);
    let send = endpoint_request(send);
    match focus.method {
        crate::api::schema::Method::PaneFocus(ref params) => {
            assert_eq!(params.pane_id, "pane_2");
        }
        other => panic!("expected pane.focus, got {other:?}"),
    }
    match send.method {
        crate::api::schema::Method::PaneSendText(params) => {
            assert_eq!(params.pane_id, "pane_2");
            assert_eq!(params.text, "maki --resume abcdefgh1234\r");
        }
        other => panic!("expected pane.send_text, got {other:?}"),
    }
}

#[test]
fn maki_sessions_enter_skips_busy_panes_and_falls_back_to_focused() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let mut snap = snapshot();
    snap.panes.push(pane_with_cwd("pane_2", "/repo/maki"));
    snap.panes.push(pane_with_cwd("pane_3", "/repo/maki"));
    state.set_snapshot(Box::new(snap));
    state.set_pane_surface(surface());
    state.overlay = Some(ClientShellOverlay::MakiSessions(maki_overlay(vec![
        session("abcdefgh1234", "/repo/maki"),
    ])));

    let outcome = press(&mut state, KeyCode::Enter);
    let [action]: [ClientShellAction; 1] = outcome.actions.try_into().expect("one action");
    let first = endpoint_request(action);
    match &first.method {
        crate::api::schema::Method::PaneProcessInfo(params) => {
            assert_eq!(params.pane_id.as_deref(), Some("pane_2"));
        }
        other => panic!("expected pane.process_info, got {other:?}"),
    }

    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &first.id,
        Ok(process_info_response("pane_2", false, vec![busy_process()])),
    );
    let [action]: [ClientShellAction; 1] = actions.try_into().expect("one action");
    let second = endpoint_request(action);
    match &second.method {
        crate::api::schema::Method::PaneProcessInfo(params) => {
            assert_eq!(params.pane_id.as_deref(), Some("pane_3"));
        }
        other => panic!("expected pane.process_info, got {other:?}"),
    }

    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &second.id,
        Ok(process_info_response("pane_3", false, vec![busy_process()])),
    );
    let [send]: [ClientShellAction; 1] = actions.try_into().expect("fallback send");
    let send = endpoint_request(send);
    match send.method {
        crate::api::schema::Method::PaneSendText(params) => {
            assert_eq!(params.pane_id, "pane_1");
            assert_eq!(params.text, "maki --resume abcdefgh1234\r");
        }
        other => panic!("expected pane.send_text, got {other:?}"),
    }
}
