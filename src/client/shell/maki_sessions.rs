use super::*;

impl ClientShellState {
    pub(super) fn open_maki_sessions_overlay(&mut self) {
        let overlay = ClientMakiSessionsOverlay {
            sessions: crate::maki_sessions::scan(),
            now: crate::maki_sessions::now_unix(),
            selected: 0,
            scroll: 0,
            confirm_delete: false,
            status: None,
        };
        self.overlay = Some(ClientShellOverlay::MakiSessions(overlay));
    }

    pub(super) fn refresh_maki_sessions(&mut self) {
        let Some(ClientShellOverlay::MakiSessions(overlay)) = self.overlay.as_mut() else {
            return;
        };
        overlay.sessions = crate::maki_sessions::scan();
        overlay.now = crate::maki_sessions::now_unix();
        overlay.selected = overlay
            .selected
            .min(overlay.sessions.len().saturating_sub(1));
        overlay.scroll = 0;
        overlay.confirm_delete = false;
    }

    pub(super) fn move_maki_sessions_selection(&mut self, delta: isize) {
        let Some(ClientShellOverlay::MakiSessions(overlay)) = self.overlay.as_mut() else {
            return;
        };
        let last = overlay.sessions.len().saturating_sub(1);
        overlay.selected = (overlay.selected as isize + delta).clamp(0, last as isize) as usize;
        overlay.confirm_delete = false;
    }

    pub(super) fn press_maki_sessions_delete(&mut self, outcome: &mut ClientShellInput) {
        let Some(ClientShellOverlay::MakiSessions(overlay)) = self.overlay.as_mut() else {
            return;
        };
        if !overlay.confirm_delete {
            overlay.confirm_delete = true;
            outcome.repaint = true;
            return;
        }
        let Some(session) = overlay.sessions.get(overlay.selected) else {
            return;
        };
        let id = session.id.clone();
        let result = crate::maki_sessions::delete(&id);
        let Some(ClientShellOverlay::MakiSessions(overlay)) = self.overlay.as_mut() else {
            return;
        };
        match result {
            Ok(()) => {
                overlay.sessions.remove(overlay.selected);
                overlay.selected = overlay
                    .selected
                    .min(overlay.sessions.len().saturating_sub(1));
                overlay.confirm_delete = false;
                overlay.status = Some(format!("deleted {id}"));
            }
            Err(err) => {
                overlay.confirm_delete = false;
                overlay.status = Some(err);
            }
        }
        outcome.repaint = true;
    }

    pub(super) fn accept_maki_sessions(&mut self, outcome: &mut ClientShellInput) {
        let session_id = match self.overlay.as_ref() {
            Some(ClientShellOverlay::MakiSessions(overlay)) => {
                let Some(session) = overlay.sessions.get(overlay.selected) else {
                    return;
                };
                if overlay.confirm_delete {
                    self.press_maki_sessions_delete(outcome);
                    return;
                }
                session.id.clone()
            }
            _ => return,
        };
        self.overlay = None;
        self.resume_maki_session(session_id, outcome);
        outcome.repaint = true;
    }

    fn resume_maki_session(&mut self, session_id: String, outcome: &mut ClientShellInput) {
        use crate::api::schema::{Method, PaneSendTextParams};

        let Some(pane_id) = self
            .snapshot
            .as_deref()
            .and_then(|snapshot| snapshot.focused_pane_id.clone())
        else {
            return;
        };
        self.push_endpoint_method(
            Method::PaneSendText(PaneSendTextParams {
                pane_id,
                text: format!("maki --resume {session_id}\r"),
            }),
            outcome,
        );
    }
}
