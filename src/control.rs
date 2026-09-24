//! Session-scoped grants and browser operations. GTK owns all WebViews.
use crate::{browser::{Browser, Tab}, control_transport::{self as transport, Incoming, Server}, core::origin};
use gtk::{gio, glib, prelude::*};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet, VecDeque}, rc::Rc, sync::mpsc, time::{Duration, Instant}};
use webkit::prelude::*;

pub struct Pending {
    reply: mpsc::Sender<Value>,
    cancel: gio::Cancellable,
    deadline: Instant,
    wait: Option<u64>,
}
pub struct Control {
    pub server: Option<Server>,
    pub session: String,
    pub grants: HashMap<String, bool>,
    pub shared: HashSet<u64>,
    pending: HashMap<String, Pending>,
    seen: HashSet<String>,
    events: VecDeque<Value>,
    sequence: u64,
}
impl Default for Control {
    fn default() -> Self { Self { server: None, session: format!("{}-{}", std::process::id(), crate::core::now()), grants: HashMap::new(), shared: HashSet::new(), pending: HashMap::new(), seen: HashSet::new(), events: VecDeque::new(), sequence: 0 } }
}
impl Browser {
    pub fn start_control(self: &Rc<Self>) {
        if self.safe_mode { self.notice("Agent control is unavailable in safe mode"); return; }
        match Server::start() {
            Ok(server) => {
                self.control.borrow_mut().server = Some(server);
                self.control_bar.set_visible(true);
                let weak = Rc::downgrade(self);
                glib::timeout_add_local(Duration::from_millis(25), move || {
                    let Some(b) = weak.upgrade() else { return glib::ControlFlow::Break; };
                    if b.closing.get() || b.control.borrow().server.is_none() { b.stop_control(); return glib::ControlFlow::Break; }
                    let incoming: Vec<_> = {
                        let state = b.control.borrow();
                        state.server.as_ref().unwrap().receiver.try_iter().take(16).collect()
                    };
                    for request in incoming { b.control_dispatch(request); }
                    let pending: Vec<_> = b.control.borrow().pending.iter().map(|(id, p)| (id.clone(), p.deadline, p.wait)).collect();
                    for (id, deadline, wait) in pending {
                        if Instant::now() >= deadline { b.control_finish(&id, Err("Request timed out; inspect state before retrying".into())); }
                        else if let Some(tab) = wait {
                            match b.control_tab(tab, false) {
                                Err(e) => b.control_finish(&id, Err(e)),
                                Ok(t) => {
                                    let ready = t.view.borrow().as_ref().is_some_and(|v| !v.is_loading());
                                    if t.failed.get() { b.control_finish(&id, Err("Page load failed".into())); }
                                    else if ready { b.control_finish(&id, Ok(json!({"loaded":true,"tab":tab}))); }
                                }
                            }
                        }
                    }
                    glib::ControlFlow::Continue
                });
            }
            Err(e) => self.notice(&e),
        }
    }
    pub fn stop_control(&self) {
        let (server, pending) = {
            let mut state = self.control.borrow_mut();
            state.grants.clear(); state.shared.clear();
            (state.server.take(), std::mem::take(&mut state.pending))
        };
        for (id, p) in pending { p.cancel.cancel(); let _ = p.reply.send(transport::failure(&id, "Agent control stopped")); }
        drop(server);
        self.control_bar.set_visible(false);
    }
    pub fn control_event(&self, kind: &str, tab: u64) {
        let mut state = self.control.borrow_mut();
        if state.server.is_none() || !state.shared.contains(&tab) { return; }
        state.sequence += 1;
        let sequence = state.sequence;
        state.events.push_back(json!({"sequence":sequence,"kind":kind,"tab":tab}));
        if state.events.len() > 512 { state.events.pop_front(); }
    }
    fn control_finish(&self, id: &str, result: Result<Value, String>) {
        let pending = self.control.borrow_mut().pending.remove(id);
        if let Some(p) = pending {
            p.cancel.cancel();
            let value = match result { Ok(value) => json!({"id":id,"session":self.control.borrow().session,"result":value}), Err(e) => transport::failure(id, &e) };
            let _ = p.reply.send(value);
        }
    }
    pub fn control_tab(&self, id: u64, interact: bool) -> Result<Rc<Tab>, String> {
        if self.control.borrow().server.is_none() { return Err("Agent control stopped".into()); }
        if !self.control.borrow().shared.contains(&id) { return Err("Tab is not shared with the agent".into()); }
        let tab = self.tabs.borrow().iter().find(|t| t.id == id && !t.private).cloned().ok_or("Tab unavailable")?;
        let uri = tab.view.borrow().as_ref().and_then(|v| v.uri()).map(|u| u.to_string()).unwrap_or_else(|| tab.page.borrow().url.clone());
        let site = origin(&uri).ok_or("Only HTTP(S) pages can be controlled")?;
        let allowed = self.control.borrow().grants.get(&site).copied();
        if allowed.is_none() || (interact && allowed != Some(true)) { return Err("Origin access is not granted; use grant and approve in Nagi".into()); }
        Ok(tab)
    }
    fn control_dispatch(self: &Rc<Self>, incoming: Incoming) {
        let Incoming { request, reply, received } = incoming;
        let id = request.id;
        {
            let mut state = self.control.borrow_mut();
            if state.server.is_none() || received.elapsed() > Duration::from_secs(2) { let _ = reply.send(transport::failure(&id, "Request expired or control stopped")); return; }
            if state.seen.len() >= 10000 || !state.seen.insert(id.clone()) { let _ = reply.send(transport::failure(&id, "Duplicate request ID or session limit reached")); return; }
            if request.params.get("session").and_then(Value::as_str).is_some_and(|s| s != state.session) { let _ = reply.send(transport::failure(&id, "Stale browser session")); return; }
            state.pending.insert(id.clone(), Pending { reply, cancel: gio::Cancellable::new(), deadline: received + Duration::from_secs(15), wait: None });
        }
        let params = request.params;
        let method = request.method.as_str();
        if method == "grant" || method == "attach" {
            let tab = if method == "attach" { self.tab().filter(|t| !t.private) } else { None };
            let site = if let Some(t) = &tab { origin(&t.page.borrow().url) } else if method == "grant" { params.get("origin").and_then(Value::as_str).and_then(origin) } else { None };
            let Some(site) = site else { self.control_finish(&id, Err("Choose a normal HTTP(S) tab or supply an origin".into())); return; };
            let tab_id = tab.map(|t| t.id);
            let dialog = gtk::AlertDialog::builder().message("Allow agent access?")
                .detail(format!("{site}\nReading can expose signed-in page content. Interaction can submit forms and perform account actions. This grant lasts until Stop or Nagi closes. Page text never grants permissions."))
                .buttons(["Deny", "Allow reading", "Allow reading and interaction"]).cancel_button(0).default_button(0).build();
            let cancel = self.control.borrow().pending[&id].cancel.clone();
            let weak = Rc::downgrade(self);
            dialog.choose(Some(&self.window), Some(&cancel), move |answer| {
                let Some(b) = weak.upgrade() else { return; };
                if !b.control.borrow().pending.contains_key(&id) { return; }
                if matches!(answer, Ok(1) | Ok(2)) {
                    b.control.borrow_mut().grants.insert(site.clone(), answer == Ok(2));
                    if let Some(tab) = tab_id { b.control.borrow_mut().shared.insert(tab); }
                    b.control_finish(&id, Ok(json!({"origin":site,"interaction":answer==Ok(2),"tab":tab_id})));
                } else { b.control_finish(&id, Err("Access denied".into())); }
            });
            return;
        }
        let result = (|| -> Result<Option<Value>, String> {
            match method {
                "capabilities" => Ok(Some(json!({"api":1,"methods":["capabilities","grant","attach","revoke","tabs","open","navigate","close","snapshot","screenshot","click","type","select","scroll","wait","events","cancel","stop"],"private_tabs":false,"arbitrary_javascript":false,"request_limit":10000,"timeout_seconds":15,"access":"per-origin, session only; approve in browser"}))),
                "tabs" => {
                    let ids: Vec<_> = self.control.borrow().shared.iter().copied().collect();
                    let list: Vec<_> = ids.into_iter().map(|n| match self.control_tab(n, false) { Ok(t) => json!({"id":n,"url":t.page.borrow().url,"title":t.page.borrow().title}), Err(_) => json!({"id":n,"access":"unavailable"}) }).collect();
                    Ok(Some(json!(list)))
                }
                "events" => {
                    let since = params.get("after").and_then(Value::as_u64).unwrap_or(0);
                    let state = self.control.borrow();
                    Ok(Some(json!({"events":state.events.iter().filter(|e| e["sequence"].as_u64().unwrap_or(0)>since).collect::<Vec<_>>(),"cursor":state.sequence,"gap":state.events.front().is_some_and(|e| e["sequence"].as_u64().unwrap_or(0)>since.saturating_add(1))})))
                }
                "revoke" => {
                    let site = params.get("origin").and_then(Value::as_str).and_then(origin).ok_or("Supply origin")?;
                    self.control.borrow_mut().grants.remove(&site);
                    let ids: Vec<_> = self.control.borrow().pending.keys().filter(|n| *n != &id).cloned().collect();
                    for pending in ids { self.control_finish(&pending, Err("Access revoked".into())); }
                    Ok(Some(json!({"revoked":site})))
                }
                "cancel" => {
                    let target = params.get("id").and_then(Value::as_str).ok_or("Supply request id")?;
                    if target == id { return Err("Cannot cancel this request itself".into()); }
                    let existed = self.control.borrow().pending.contains_key(target);
                    self.control_finish(target, Err("Cancelled; completed page effects are not undone".into()));
                    Ok(Some(json!({"cancelled":existed})))
                }
                "stop" => { self.control_finish(&id, Ok(json!({"stopped":true}))); self.stop_control(); Ok(None) }
                "open" => {
                    let uri = params.get("url").and_then(Value::as_str).ok_or("Supply url")?;
                    crate::personal::http_url(uri)?;
                    let site = origin(uri).ok_or("Invalid origin")?;
                    if !self.control.borrow().grants.contains_key(&site) { return Err("Origin access not granted".into()); }
                    let tab = self.new_tab(uri, false, true);
                    self.control.borrow_mut().shared.insert(tab.id);
                    self.control_event("tab.opened", tab.id);
                    Ok(Some(json!({"tab":tab.id})))
                }
                _ => {
                    let tab_id = params.get("tab").and_then(Value::as_u64).ok_or("Supply numeric tab")?;
                    let mutation = ["navigate","close","click","type","select","scroll"].contains(&method);
                    let tab = self.control_tab(tab_id, mutation)?;
                    match method {
                        "close" => { self.control_event("tab.closed", tab_id); self.close_tab(tab_id); self.control.borrow_mut().shared.remove(&tab_id); Ok(Some(json!({"closed":true}))) }
                        "navigate" => {
                            let uri = params.get("url").and_then(Value::as_str).ok_or("Supply url")?;
                            crate::personal::http_url(uri)?;
                            if self.control.borrow().grants.get(&origin(uri).ok_or("Invalid origin")?).copied() != Some(true) { return Err("Destination interaction access not granted".into()); }
                            self.select(tab_id); self.navigate(uri);
                            Ok(Some(json!({"navigation_started":true})))
                        }
                        "wait" => { self.control.borrow_mut().pending.get_mut(&id).unwrap().wait = Some(tab_id); Ok(None) }
                        "snapshot" | "click" | "type" | "select" | "scroll" | "screenshot" => {
                            let view = tab.view.borrow().clone().ok_or("Page is not loaded")?;
                            if view.is_loading() { return Err("Page is loading; wait before taking an observation or action".into()); }
                            if ["click","type","select"].contains(&method) && params.get("ref").and_then(Value::as_str).is_none() { return Err("Supply element ref from snapshot".into()); }
                            if method == "type" && params.get("text").and_then(Value::as_str).is_none() { return Err("Supply text".into()); }
                            if method == "select" && params.get("value").and_then(Value::as_str).is_none() { return Err("Supply option value".into()); }
                            if method == "scroll" && ["x","y"].iter().any(|k| params.get(k).is_some_and(|v| v.as_f64().is_none_or(|n| n.abs()>10000.0))) { return Err("Scroll coordinates must be numbers within ±10000".into()); }
                            let generation = tab.generation.get();
                            let cancel = self.control.borrow().pending[&id].cancel.clone();
                            let weak = Rc::downgrade(self);
                            let request_id = id.clone();
                            if method == "screenshot" {
                                view.snapshot(webkit::SnapshotRegion::Visible, webkit::SnapshotOptions::NONE, Some(&cancel), move |result| {
                                    if let Some(b) = weak.upgrade() {
                                        let result = b.control_observation_valid(tab_id, generation).and_then(|_| {
                                            let surface = result.map_err(|e| e.to_string())?;
                                            let mut bytes = Vec::new();
                                            surface.write_to_png(&mut bytes).map_err(|e| e.to_string())?;
                                            if bytes.len() > 8 * 1024 * 1024 { return Err("Screenshot exceeds 8 MiB".into()); }
                                            Ok(json!({"mime":"image/png","base64":glib::base64_encode(&bytes),"untrusted":true}))
                                        });
                                        b.control_finish(&request_id, result);
                                    }
                                });
                            } else {
                                let script = include_str!("../assets/control.js").replace("__NAGI_REQUEST__", &json!({"method":method,"params":params}).to_string());
                                view.evaluate_javascript(&script, Some("nagi-control"), None, Some(&cancel), move |result| {
                                    if let Some(b) = weak.upgrade() {
                                        let result = b.control_observation_valid(tab_id, generation).and_then(|_| {
                                            let value = result.map_err(|e| e.to_string())?;
                                            serde_json::from_str(value.to_str().as_str()).map_err(|e| e.to_string())
                                        });
                                        b.control_finish(&request_id, result);
                                    }
                                });
                            }
                            self.control_event(if mutation { "agent.action" } else { "agent.observation" }, tab_id);
                            Ok(None)
                        }
                        _ => Err("Unknown method; run capabilities".into()),
                    }
                }
            }
        })();
        match result { Ok(Some(value)) => self.control_finish(&id, Ok(value)), Err(e) => self.control_finish(&id, Err(e)), Ok(None) => {} }
    }
    fn control_observation_valid(&self, tab: u64, generation: u64) -> Result<(), String> {
        let tab = self.control_tab(tab, false)?;
        if tab.generation.get() != generation { return Err("Page navigated; discard this result and observe again".into()); }
        Ok(())
    }
}
