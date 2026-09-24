//! Import only bookmarks from a browser's exported JSON. No cookies, tokens,
//! passwords or profile data are read or copied.
use crate::core::Page;
use serde_json::Value;

pub fn bookmarks(bytes: &[u8]) -> Result<Vec<Page>, String> {
    if bytes.len() > 5_000_000 {
        return Err("Bookmark export exceeds 5 MB".into());
    }
    let json: Value = serde_json::from_slice(bytes).map_err(|_| "Invalid bookmarks JSON")?;
    let mut pages = Vec::new();
    if let Some(list) = json.as_array() {
        for value in list {
            let page: Page =
                serde_json::from_value(value.clone()).map_err(|_| "Invalid Nagi bookmark")?;
            append(&mut pages, &page.url, &page.title);
        }
    } else if let Some(roots) = json.get("roots") {
        for node in ["bookmark_bar", "other", "synced"] {
            if let Some(value) = roots.get(node) {
                walk(value, &mut pages, 0);
            }
        }
    } else {
        return Err("Expected Nagi bookmarks or a Chromium/Comet Bookmarks export".into());
    }
    Ok(pages)
}
fn walk(value: &Value, out: &mut Vec<Page>, depth: usize) {
    if depth > 32 || out.len() >= 10_000 {
        return;
    }
    if value.get("type").and_then(Value::as_str) == Some("url") {
        if let Some(url) = value.get("url").and_then(Value::as_str) {
            append(
                out,
                url,
                value.get("name").and_then(Value::as_str).unwrap_or(url),
            );
        }
    } else if let Some(children) = value.get("children").and_then(Value::as_array) {
        for child in children {
            walk(child, out, depth + 1);
        }
    }
}
fn append(out: &mut Vec<Page>, url: &str, title: &str) {
    if out.len() >= 10_000 || out.iter().any(|p| p.url == url) {
        return;
    }
    if url::Url::parse(url)
        .ok()
        .is_some_and(|u| ["http", "https"].contains(&u.scheme()))
    {
        out.push(Page {
            url: url.into(),
            title: title.into(),
            pinned: false,
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_only_safe_unique_chromium_bookmarks() {
        let json = br#"{"roots":{"bookmark_bar":{"children":[{"type":"url","name":"A","url":"https://example.com"},{"type":"url","name":"B","url":"javascript:alert(1)"},{"type":"folder","children":[{"type":"url","name":"C","url":"https://example.com"}]}]}},"passwords":["secret"]}"#;
        let pages = bookmarks(json).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title, "A");
    }
}
