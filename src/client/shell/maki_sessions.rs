use super::*;

/// A pane is a valid maki resume target only when its shell sits at the prompt
/// with nothing running in it.
pub(super) fn maki_probe_pane_is_idle(process_info: &crate::api::schema::PaneProcessInfo) -> bool {
    match process_info.shell_idle {
        Some(idle) => idle,
        // Older servers do not report shell_idle: on them the idle shell is
        // itself the foreground job, so a job consisting only of the shell
        // process still means the pane is idle.
        None => match process_info.shell_pid {
            Some(shell_pid) => process_info
                .foreground_processes
                .iter()
                .all(|process| process.pid == shell_pid),
            None => process_info.foreground_processes.is_empty(),
        },
    }
}

/// Compare a pane cwd against the maki session cwd. Component-wise comparison
/// normalizes separator differences, and Windows path components compare
/// case-insensitively.
fn maki_cwd_matches(pane_cwd: &str, session_cwd: &str) -> bool {
    let pane_components: Vec<_> = std::path::Path::new(pane_cwd)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let session_components: Vec<_> = std::path::Path::new(session_cwd)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    if pane_components.len() != session_components.len() {
        return false;
    }
    pane_components
        .iter()
        .zip(session_components.iter())
        .all(|(pane, session)| {
            if cfg!(windows) {
                pane.eq_ignore_ascii_case(session)
            } else {
                pane == session
            }
        })
}

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
        let (session_id, session_cwd) = match self.overlay.as_ref() {
            Some(ClientShellOverlay::MakiSessions(overlay)) => {
                let Some(session) = overlay.sessions.get(overlay.selected) else {
                    return;
                };
                if overlay.confirm_delete {
                    self.press_maki_sessions_delete(outcome);
                    return;
                }
                (session.id.clone(), session.cwd.clone())
            }
            _ => return,
        };
        self.overlay = None;
        self.begin_maki_resume(session_id, session_cwd, outcome);
        outcome.repaint = true;
    }

    /// Resume a maki session in the best matching pane: prefer an existing pane
    /// whose cwd matches the session cwd and that has no foreground job. When no
    /// such pane exists, fall back to the focused pane.
    fn begin_maki_resume(
        &mut self,
        session_id: String,
        session_cwd: String,
        outcome: &mut ClientShellInput,
    ) {
        use crate::api::schema::{Method, PaneProcessInfoParams};

        // Older servers do not advertise pane.process_info; probing would only
        // surface an unsupported-method notice, so fall back to the focused
        // pane without one.
        let probe = Method::PaneProcessInfo(PaneProcessInfoParams { pane_id: None });
        if !self.supports_endpoint_method(&probe) {
            self.resume_maki_session(session_id, outcome);
            return;
        }
        let Some(candidate) = self.maki_resume_candidate_panes(&session_cwd) else {
            self.resume_maki_session(session_id, outcome);
            return;
        };
        let (pane_id, remaining) = candidate;
        let pushed = self.push_endpoint_method_with_kind(
            Method::PaneProcessInfo(PaneProcessInfoParams {
                pane_id: Some(pane_id.clone()),
            }),
            PendingEndpointKind::MakiResumeProbe {
                session_id: session_id.clone(),
                pane_id,
                remaining,
            },
            outcome,
        );
        if !pushed {
            self.resume_maki_session(session_id, outcome);
        }
    }

    fn maki_resume_candidate_panes(&self, session_cwd: &str) -> Option<(String, Vec<String>)> {
        let snapshot = self.snapshot.as_deref()?;
        let mut candidates = snapshot
            .panes
            .iter()
            .filter(|pane| {
                pane.cwd
                    .as_deref()
                    .is_some_and(|cwd| maki_cwd_matches(cwd, session_cwd))
                    || pane
                        .foreground_cwd
                        .as_deref()
                        .is_some_and(|cwd| maki_cwd_matches(cwd, session_cwd))
            })
            .map(|pane| pane.pane_id.clone());
        let first = candidates.next()?;
        Some((first, candidates.collect()))
    }

    pub(super) fn complete_maki_resume_probe(
        &mut self,
        session_id: String,
        pane_id: String,
        remaining: Vec<String>,
        idle: bool,
        outcome: &mut ClientShellInput,
    ) {
        use crate::api::schema::{Method, PaneProcessInfoParams};

        if idle {
            self.send_maki_resume(&pane_id, true, &session_id, outcome);
            return;
        }
        let Some((next, rest)) = remaining.split_first() else {
            self.resume_maki_session(session_id, outcome);
            return;
        };
        let probe = Method::PaneProcessInfo(PaneProcessInfoParams {
            pane_id: Some(next.clone()),
        });
        if !self.supports_endpoint_method(&probe) {
            self.send_maki_resume(next, true, &session_id, outcome);
            return;
        }
        let pushed = self.push_endpoint_method_with_kind(
            probe,
            PendingEndpointKind::MakiResumeProbe {
                session_id: session_id.clone(),
                pane_id: next.clone(),
                remaining: rest.to_vec(),
            },
            outcome,
        );
        if !pushed {
            self.send_maki_resume(next, true, &session_id, outcome);
        }
    }

    fn resume_maki_session(&mut self, session_id: String, outcome: &mut ClientShellInput) {
        let Some(pane_id) = self
            .snapshot
            .as_deref()
            .and_then(|snapshot| snapshot.focused_pane_id.clone())
        else {
            return;
        };
        self.send_maki_resume(&pane_id, false, &session_id, outcome);
    }

    fn send_maki_resume(
        &mut self,
        pane_id: &str,
        focus: bool,
        session_id: &str,
        outcome: &mut ClientShellInput,
    ) {
        use crate::api::schema::{Method, PaneSendTextParams, PaneTarget};

        if focus {
            self.push_endpoint_method(
                Method::PaneFocus(PaneTarget {
                    pane_id: pane_id.to_owned(),
                }),
                outcome,
            );
        }
        self.push_endpoint_method(
            Method::PaneSendText(PaneSendTextParams {
                pane_id: pane_id.to_owned(),
                text: format!("maki --resume {session_id}\r"),
            }),
            outcome,
        );
    }
}

#[cfg(test)]
mod maki_cwd_tests {
    use super::maki_cwd_matches;

    #[test]
    fn separator_and_case_differences_do_not_break_matches() {
        if cfg!(windows) {
            assert!(maki_cwd_matches(r"D:\code\herdr", r"d:/code/herdr"));
            assert!(!maki_cwd_matches(r"D:\code\herdr", r"D:\code\herdr\sub"));
        } else {
            assert!(maki_cwd_matches("/repo/maki", "/repo/maki"));
            assert!(!maki_cwd_matches("/repo/maki", "/repo/maki/sub"));
            assert!(!maki_cwd_matches("/repo/Maki", "/repo/maki"));
        }
    }
}
