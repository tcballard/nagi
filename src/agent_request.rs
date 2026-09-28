//! Native owner requests spawn the restricted adapter, never a command shell.
use crate::browser::Browser;
use gtk::{gio, glib, prelude::*};
use std::{ffi::OsStr, rc::Rc};

impl Browser {
    pub fn ask_agent(self: &Rc<Self>) {
        let task = self.address.text().trim().to_string();
        let fail = |message: &str| {
            self.address_error.set_text(message);
            self.address_error.set_visible(true);
        };
        if self.safe_mode {
            fail("Agent requests are unavailable in safe mode.");
            return;
        }
        if task.is_empty() || task.len() > 8192 {
            fail("Describe a settings change in 1–8192 bytes.");
            return;
        }
        if self.agent_request.borrow().is_some() {
            fail("An agent request is already running. Use Stop agent control to cancel it.");
            return;
        }
        self.expire_agent_proposal();
        let control = self.control.borrow();
        if control.server.is_none() {
            fail("Start Nagi with --agent-control to use Ask agent.");
            return;
        }
        if control.proposal.is_some() {
            fail("Review the pending agent change before making another request.");
            return;
        }
        let session = control.session.clone();
        drop(control);
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(_) => {
                fail("Could not locate Nagi. Reinstall Nagi and try again.");
                return;
            }
        };
        let sibling = executable.with_file_name("nagi-codex");
        let adapter = if sibling.is_file() {
            Some(sibling)
        } else {
            glib::find_program_in_path("nagi-codex")
        };
        let Some(adapter) = adapter else {
            fail("Install nagi-codex and sign in with codex login first.");
            return;
        };
        let argv: [&OsStr; 7] = [
            adapter.as_os_str(),
            OsStr::new("--settings-only"),
            OsStr::new("--task-stdin"),
            OsStr::new("--nagi"),
            executable.as_os_str(),
            OsStr::new("--session"),
            OsStr::new(&session),
        ];
        let child = match gio::Subprocess::newv(
            &argv,
            gio::SubprocessFlags::STDIN_PIPE
                | gio::SubprocessFlags::STDOUT_PIPE
                | gio::SubprocessFlags::STDERR_PIPE,
        ) {
            Ok(child) => child,
            Err(_) => {
                fail("Could not start nagi-codex. Check that it and Codex CLI are installed.");
                return;
            }
        };
        *self.agent_request.borrow_mut() = Some(child.clone());
        self.dismiss_address();
        self.notice("Preparing your settings proposal… Use Stop agent control to cancel.");
        let weak = Rc::downgrade(self);
        let running = child.clone();
        child.communicate_utf8_async(Some(task), gio::Cancellable::NONE, move |result| {
            let Some(b) = weak.upgrade() else { return };
            // Stop/close removes the handle first; a late completion cannot
            // announce success in a stopped or replacement control session.
            if b.agent_request.borrow().as_ref() != Some(&running) {
                return;
            }
            b.agent_request.borrow_mut().take();
            if b.closing.get() || b.control.borrow().session != session {
                return;
            }
            let text = match result {
                Ok((stdout, _stderr)) if running.is_successful() => {
                    stdout.map(|s| s.to_string()).unwrap_or_default()
                }
                Ok((_, stderr)) => stderr
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Agent request failed. No change was approved.".into()),
                Err(_) => "Agent request failed. No change was approved.".into(),
            };
            let bounded: String = text.trim().chars().take(1200).collect();
            b.notice(&bounded);
        });
    }

    pub fn cancel_agent_request(&self) {
        if let Some(child) = self.agent_request.borrow_mut().take() {
            // SIGTERM lets the trusted adapter clean its private login copy and
            // terminate its app-server process group. This never approves UI.
            child.send_signal(15);
        }
    }
}
