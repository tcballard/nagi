//! Proposals carry no approval authority. Only the native owner UI can commit.
use crate::{browser::Browser, config, config_store, core::Settings, extensions};
use gtk::gio;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Proposal {
    pub id: String,
    revision: u64,
    reason: String,
    detail: String,
    before: Settings,
    next: Settings,
    extension: Option<(String, String)>,
    expires: Instant,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    session: String,
    revision: u64,
    reason: String,
    ttl_seconds: Option<u64>,
    changes: Option<BTreeMap<String, Value>>,
    extension: Option<ExtensionCommand>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionCommand {
    id: String,
    command: String,
}
impl Browser {
    pub fn propose_agent_settings(
        &self,
        request_id: &str,
        params: &Value,
    ) -> Result<Value, String> {
        self.expire_agent_proposal();
        let input: Input = serde_json::from_value(params.clone()).map_err(|e| e.to_string())?;
        let ttl = input.ttl_seconds.unwrap_or(300);
        if !(1..=300).contains(&ttl) {
            return Err("Proposal lifetime must be 1–300 seconds".into());
        }
        let state = self.control.borrow();
        if self.safe_mode || state.server.is_none() || input.session != state.session {
            return Err("Proposal requires a current control session".into());
        }
        if state.proposal.is_some() {
            return Err("An agent proposal is already awaiting review".into());
        }
        if input.reason.trim().is_empty()
            || input.reason.len() > 512
            || input.reason.chars().any(char::is_control)
        {
            return Err(
                "Supply a plain-text reason of 1–512 bytes without control characters".into(),
            );
        }
        let (changes, extension) = match (input.changes, input.extension) {
            (Some(changes), None) => (changes, None),
            (None, Some(command)) => {
                let installed = extensions::load(&command.id)?;
                if !installed.enabled {
                    return Err("Extension is not approved".into());
                }
                let action = installed
                    .manifest
                    .commands
                    .iter()
                    .find(|c| c.id == command.command)
                    .ok_or("Unknown extension command")?;
                let extensions::Action::Configure { changes } = &action.action else {
                    return Err("Command is not a settings change".into());
                };
                (
                    changes
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                    Some((command.id, installed.digest)),
                )
            }
            _ => return Err("Supply exactly one of changes or extension".into()),
        };
        if changes.is_empty() || changes.len() > 32 {
            return Err("Supply 1–32 settings".into());
        }
        let initial = config::document()?;
        let preview = config_store::transact(
            &config::path(),
            &initial.settings,
            Some(input.revision),
            true,
            |d| {
                for (key, value) in &changes {
                    config::set(
                        &mut d.settings,
                        key,
                        &value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    )?;
                }
                Ok(())
            },
        )?;
        let mut lines = Vec::new();
        for key in changes.keys() {
            let before = config::value_for_key(&initial.settings, key)?;
            let after = config::value_for_key(&preview.settings, key)?;
            if before != after {
                lines.push(format!("{key}: {before} → {after}"));
            }
        }
        if lines.is_empty() {
            return Err("Proposal would not change settings".into());
        }
        let id = format!("{}:{request_id}", state.session);
        let detail = format!("Agent-provided reason (untrusted):\n{}\n\n{}\n\nReview these values. Undo is available in Settings.", input.reason, lines.join("\n"));
        if detail.len() > 4096 {
            return Err("Proposal is too long to preview safely".into());
        }
        drop(state);
        config_store::transact_audited(
            &config::path(),
            &initial.settings,
            Some(input.revision),
            false,
            Some((&id, &input.reason, "proposed")),
            |_| Ok(()),
        )?;
        self.control.borrow_mut().proposal = Some(Proposal {
            id: id.clone(),
            revision: input.revision,
            reason: input.reason,
            detail,
            before: initial.settings,
            next: preview.settings,
            extension,
            expires: Instant::now() + Duration::from_secs(ttl),
        });
        self.notice("Agent change awaiting review — use Review agent change");
        Ok(json!({"proposal":id,"status":"awaiting_owner","expires_in_seconds":ttl}))
    }
    pub fn discard_agent_proposal(&self, outcome: &str) {
        let proposal = self.control.borrow_mut().proposal.take();
        if let Some(p) = proposal {
            let result = config::document().and_then(|d| {
                config_store::transact_audited(
                    &config::path(),
                    &d.settings,
                    None,
                    false,
                    Some((&p.id, &p.reason, outcome)),
                    |_| Ok(()),
                )
            });
            if let Err(e) = result {
                self.notice(&format!("Proposal revoked; audit update failed: {e}"));
            }
        }
    }
    pub fn expire_agent_proposal(&self) {
        let expired = self
            .control
            .borrow()
            .proposal
            .as_ref()
            .is_some_and(|p| Instant::now() >= p.expires);
        if expired {
            self.discard_agent_proposal("expired");
        }
    }
    pub fn review_agent_proposal(self: &Rc<Self>) {
        self.expire_agent_proposal();
        let Some(proposal) = self.control.borrow().proposal.clone() else {
            self.notice("No agent change awaiting review");
            return;
        };
        let dialog = gtk::AlertDialog::builder()
            .message("Apply this agent proposal?")
            .detail(&proposal.detail)
            .buttons(["Reject proposal", "Apply proposal"])
            .cancel_button(0)
            .default_button(0)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window), gio::Cancellable::NONE, move |answer| {
            let Some(b) = weak.upgrade() else {
                return;
            };
            b.expire_agent_proposal();
            let current = b.control.borrow();
            if current.server.is_none()
                || current
                    .proposal
                    .as_ref()
                    .is_none_or(|p| p.id != proposal.id)
            {
                b.notice("Proposal expired or was revoked; no settings changed");
                return;
            }
            drop(current);
            if answer != Ok(1) {
                b.discard_agent_proposal("rejected");
                return;
            }
            let result = (|| {
                if let Some((id, digest)) = &proposal.extension {
                    let installed = extensions::load(id)?;
                    if !installed.enabled || installed.digest != *digest {
                        return Err("Extension changed since proposal".into());
                    }
                }
                config_store::transact_audited(
                    &config::path(),
                    &proposal.before,
                    Some(proposal.revision),
                    false,
                    Some((&proposal.id, &proposal.reason, "applied")),
                    |d| {
                        d.settings = proposal.next.clone();
                        Ok(())
                    },
                )
            })();
            match result {
                Ok(d) => {
                    b.control.borrow_mut().proposal = None;
                    b.apply_settings(d.settings);
                    b.notice("Agent change applied; use Settings to undo");
                }
                Err(e) => {
                    b.discard_agent_proposal("failed");
                    b.notice(&e);
                }
            }
        });
    }
}
