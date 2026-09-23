# Verification — Nagi v0.0.2

Target: Omarchy 4 / Hyprland. An actual Omarchy desktop has not been exercised.
This is an early versioned preview; see the release notes for feature limits.

## Floating address bar development

Based on v0.0.2 source `dfcbf66d47b528b5a3da2660e6d8880b6d8f84e9`.
The address/navigation controls now live in a centred GTK overlay. The GUI
fixture checks both shortcuts, Enter navigation, Escape and outside-click
dismissal with page focus restored, and single-instance `--focus-address`
without adding a tab. It captures normal and narrow-window layouts.

Current workspace has no GTK/WebKit development environment. New build and
GUI results must come from this branch's CI; the release results below are
historical and do not validate this change. Actual Hyprland interception,
desktop-wide activation, IME and fractional scaling remain untested.

Omarchy development binding source inspected at
`b9ddccfc377abe0b8fc3ff1ee5b31a86bf202d4a`: `config/hypr/bindings.lua`,
`default/hypr/bindings/{utilities,tiling-v2,clipboard,media}.lua` and
`default/hypr/plain-bindings.lua`. Super+Alt+L is unused in those sources;
the user's installed version and custom bindings have not been inspected.

## Historical icon update

[PR #1's workflow](https://github.com/tcballard/nagi/actions/runs/35920157957) passed at `5474d89f79e45bd23006a493ac4f23aacf6d4e72`: approved icon pixel checks, all nine installed sizes through GTK lookup, symbolic discovery, browser smoke tests, and Arch packaging/install/removal. The v0.0.2 release workflow repeats these checks on its own target before publication.

## Original browser checks

The tested source is available at [the v0.0.1 workflow run](https://github.com/tcballard/nagi/actions/runs/35917741980). Its source revision is `57a18bf403911c6e46b23a083fa2e1ad7a34046d`. The release workflow rebuilds and tests the tagged target before publishing assets.

- Locally on Ubuntu 24.04 x86_64: Rust 1.98.1, GTK 4.14.5, WebKitGTK 2.52.3; `cargo fmt --check`, `cargo test --locked` (six core tests), `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --release --locked`, `nagi --version` and `desktop-file-validate` pass.
- The CI GUI smoke test launches the real GTK/WebKit browser under Xvfb with WebKit's sandbox enabled. It renders a local HTTP page, saves a bookmark, invokes find and reading view, excludes private navigation from saved records, reopens a tab, saves a download via GTK's file dialog, quits, reopens and compares saved tabs. [Test screenshot](docs/ci-browser.png) is from the original browser run, before the icon update, captured on Ubuntu X11/Xvfb with a local fixture. It is not an Omarchy screenshot.
- Arch Linux container: the workflow generates a source tarball from the commit and a PKGBUILD with its actual SHA-256; `makepkg` runs the locked build and tests as an ordinary user; the package is inspected, installed with pacman, reports the correct version, supplies the desktop file, then is removed. This is an Arch container check, not a clean chroot or a live Omarchy install.

The local workspace cannot create a desktop socket, so it cannot launch the browser's GUI here. CI provides the runtime evidence above. Check the linked workflow's final result for the exact source revision; a partial job is not a pass.

## Still requires real Omarchy acceptance

Wayland / Hyprland window identity and portals; Omarchy palette switching with installed themes; fractional scaling, IME, clipboard and multiple monitors; authenticated sites; camera/audio permissions and media playback; package upgrade and removal on the target machine. The `aarch64` recipe is declared but only x86_64 is built here. Content blocking, element hiding and site permissions are implemented but do not yet have live Omarchy acceptance.

For the first device pass: install the release package, launch from the app menu, open several HTTPS pages, switch the current Omarchy theme, try private tabs and a download, quit/reopen to check restore, then uninstall with `sudo pacman -R nagi`. Keep your existing default browser during this pass.
