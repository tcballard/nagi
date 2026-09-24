use crate::core::{Page, State};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind { Tab(u64), Bookmark, History }
#[derive(Clone, Debug)]
pub struct Suggestion { pub title: String, pub url: String, pub kind: Kind }
impl Suggestion {
    pub fn caption(&self) -> &str {
        match self.kind { Kind::Tab(_) => "Switch to tab", Kind::Bookmark => "Bookmark", Kind::History => "History" }
    }
}
pub fn find(query: &str, state: &State, tabs: &[(u64, Page, bool)], private: bool) -> Vec<Suggestion> {
    let query = query.trim().to_lowercase();
    if query.is_empty() { return vec![]; }
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    let mut add = |title: &str, url: &str, kind: Kind| {
        let text = format!("{title} {url}").to_lowercase();
        if results.len() < 6 && url != "about:blank" && query.split_whitespace().all(|q| text.contains(q)) && seen.insert(url.to_string()) {
            results.push(Suggestion { title: if title.is_empty() { url.into() } else { title.into() }, url: url.into(), kind });
        }
    };
    for (id, page, is_private) in tabs {
        if *is_private == private { add(&page.title, &page.url, Kind::Tab(*id)); }
    }
    for page in &state.bookmarks { add(&page.title, &page.url, Kind::Bookmark); }
    if !private {
        for visit in &state.history { add(&visit.title, &visit.url, Kind::History); }
    }
    results
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suggestions_are_local_deduplicated_and_private() {
        let page = Page { title: "Quiet sea".into(), url: "https://sea.test".into(), pinned: false };
        let secret = Page { title: "Quiet secret".into(), url: "https://secret.test".into(), pinned: false };
        let mut state = State::default();
        state.bookmarks.push(page.clone());
        state.visit("https://history.test", "Quiet history");
        let tabs = vec![(1, page, false), (2, secret, true)];
        let normal = find("QUIET", &state, &tabs, false);
        assert_eq!(normal.len(), 2);
        assert_eq!(normal[0].kind, Kind::Tab(1));
        let private = find("quiet", &state, &tabs, true);
        assert_eq!(private.len(), 2);
        assert_eq!(private[0].kind, Kind::Tab(2));
        assert!(private.iter().all(|s| s.kind != Kind::History));
        assert!(find("   ", &state, &tabs, false).is_empty());
        assert!(find("quiet sea", &state, &tabs, false).len() == 1);
    }
    #[test]
    fn old_state_restores_default_geometry_and_bounds_invalid_sizes() {
        let state: State = serde_json::from_str(r#"{"schema":1}"#).unwrap();
        assert_eq!(state.window.size(), (1180, 800));
        let state: State = serde_json::from_str(r#"{"window":{"width":-1,"height":99999}}"#).unwrap();
        assert_eq!(state.window.size(), (320, 4320));
    }
}
