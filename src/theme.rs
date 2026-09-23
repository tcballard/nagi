use crate::core::xdg;
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub bg: String,
    pub fg: String,
    pub accent: String,
    pub surface: String,
    pub dark: bool,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            bg: "#191d22".into(),
            fg: "#e5e8e2".into(),
            accent: "#b4dc81".into(),
            surface: "#252b31".into(),
            dark: true,
        }
    }
}
fn color(v: &str) -> bool {
    v.len() == 7 && v.starts_with('#') && v[1..].bytes().all(|c| c.is_ascii_hexdigit())
}
pub fn parse(text: &str) -> Option<Palette> {
    let t: toml::Value = toml::from_str(text).ok()?;
    let get = |k: &str| t.get(k)?.as_str().filter(|v| color(v)).map(str::to_owned);
    let bg = get("background")?;
    let fg = get("foreground")?;
    let accent = get("accent")?;
    let rgb = u32::from_str_radix(&bg[1..], 16).ok()?;
    let dark = ((rgb >> 16) as f64 * 0.2126
        + ((rgb >> 8) & 255) as f64 * 0.7152
        + (rgb & 255) as f64 * 0.0722)
        < 140.0;
    let surface = get("color0").unwrap_or_else(|| bg.clone());
    Some(Palette {
        bg,
        fg,
        accent,
        surface,
        dark,
    })
}
pub fn load() -> Option<Palette> {
    parse(
        &std::fs::read_to_string(
            xdg("XDG_CONFIG_HOME", ".config").join("omarchy/current/theme/colors.toml"),
        )
        .ok()?,
    )
}
pub fn css(p: &Palette) -> String {
    format!(
        r#"
window.browser {{ background: {bg}; color: {fg}; }}
.browser .chrome, .browser .tab-strip {{ background: {bg}; color: {fg}; }}
.browser button {{ border-radius: 8px; box-shadow: none; min-height: 26px; }}
.browser .chrome button, .browser .tab-strip button {{ background: transparent; border: none; color: {fg}; }}
.browser .chrome button:hover, .browser .tab-strip button:hover {{ background: alpha({fg},0.10); }}
.browser entry {{ background: alpha({fg},0.06); color: {fg}; border: 1px solid alpha({fg},0.12); border-radius: 9px; box-shadow: none; min-height: 32px; }}
.browser entry:focus-within {{ border-color: {accent}; }}
.browser .tab {{ border-radius: 9px; padding: 2px 3px; margin: 3px 0; }}
.browser .tab.active {{ background: alpha({fg},0.10); }}
.browser .tab.private {{ border-bottom: 2px solid {accent}; }}
.browser .tab label {{ font-size: 12px; }}
.browser .panel {{ background: {bg}; border-left: 1px solid alpha({fg},0.12); padding: 18px; }}
.browser .panel row {{ padding: 8px 2px; border-bottom: 1px solid alpha({fg},0.06); }}
.browser .panel row:hover {{ background: alpha({fg},0.06); }}
.browser .muted {{ opacity: 0.65; font-size: 12px; }}
.browser .heading {{ font-size: 23px; font-weight: 600; }}
.browser .brand {{ font-size: 36px; font-weight: 600; letter-spacing: -1px; }}
.browser .eyebrow {{ color: {accent}; font-size: 11px; font-weight: bold; letter-spacing: 3px; }}
.browser .welcome {{ padding: 40px; }}
.browser .welcome button {{ background: alpha({fg},0.06); color: {fg}; border: 1px solid alpha({fg},0.10); padding: 12px 22px; }}
.browser progressbar trough {{ min-height: 2px; border: none; background: transparent; }}
.browser progressbar progress {{ background: {accent}; border: none; min-height: 2px; min-width: 0; margin: 0; padding: 0; }}
.browser .notice {{ background: {surface}; padding: 8px 14px; }}
.browser .status {{ font-size: 11px; padding: 3px 10px; opacity: 0.75; }}
"#,
        bg = p.bg,
        fg = p.fg,
        accent = p.accent,
        surface = p.surface
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_css_injection_and_accepts_light_theme() {
        assert!(parse("background = 'red; }'\nforeground='#ffffff'\naccent='#ffffff'").is_none());
        assert!(
            !parse("background='#ffffff'\nforeground='#000000'\naccent='#123456'")
                .unwrap()
                .dark
        );
    }
}
