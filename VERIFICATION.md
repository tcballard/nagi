# Verification — Nagi

Target: Omarchy 4 / Hyprland. An actual Omarchy desktop has not been exercised.
This is an early versioned preview; see the release notes for feature limits.

## Composer and browser polish

Runtime source: `b7c82285fa1753b1604babe9be1b89708bf6ac83`.
[Workflow 35988075693](https://github.com/tcballard/nagi/actions/runs/35988075693)
checks the current polish. The final documentation commit adds captures and
notes only; it does not change the tested source.

Linux passes formatting, eight Rust tests, debug/release builds, approved-icon
pixel checks, desktop validation and the real GTK/Xvfb browser fixture.
The fixture exercises local tab switching and history selection, unchanged
plain-Enter navigation, page-focus restoration, normal/private session
separation, native downloads, a 520px-wide composer and a real resize/restart
that restores 1040 × 720. A second launch disables GTK animations and checks
composer navigation and dismissal. A screenshot pixel assertion verifies both
loaded tabs retain their website favicon after same-site navigation.

Pure tests cover case-insensitive matching, URL deduplication, tab priority,
private-tab isolation, exclusion of normal history from private suggestions,
explicit bookmarks in private mode, empty input, old state defaults and invalid
window-size bounds. The Arch job also passed its ordinary-user package build,
tests, pacman installation, version/desktop checks and removal.
Suggestions use local records only. Private website icons
use the private WebKit network session.

Visually inspected actual Ubuntu/X11 captures:
[local suggestions](docs/polish-suggestions.png),
[quiet new tab](docs/polish-new-tab.png),
[narrow composer](docs/polish-narrow.png), and
[cached site icons](docs/polish-favicons.png).
The green squares are the fixture website's favicon, not replacement app icons.

The 120ms crossfade uses GtkRevealer and GTK's animation preference. Maximized
state is saved/restored through GTK; maximized/fullscreen behavior under a real
window manager was not exercised by Xvfb. Live Hyprland, fractional scaling,
IME and accessibility remain device checks. This work stays in PR #2 and does
not publish a new version.

Capture SHA-256:

- `polish-suggestions.png`: `10a7287dfacb0ad0a043233363f8237bcfa87aab31c90649e070846fd59224f8`
- `polish-new-tab.png`: `0986415464f67dcece5c17eb6b7b8185416355c92870aea3a83ba6aaae63d9f3`
- `polish-narrow.png`: `415858bfce11697a3d8fe3305df45d6db42856d25e7504a16bd0fa29a99faef7`
- `polish-favicons.png`: `b9863e0322a850ddc0acbcdfb60d787e81d72b75c5406a814cdde285103e88c2`

## Search composer refinement (historical)

Runtime source: `84df0d0a429f5806636d660ad19230a4236358bd`.
[Workflow 35932049027](https://github.com/tcballard/nagi/actions/runs/35932049027)
contains the checks for this revision; earlier runs below are historical.
The Linux job passed: formatting, six Rust tests, debug/release builds, icon and
desktop validation, and the real GTK/Xvfb fixture. Screenshots were visually
checked at 1180px and 520px window widths. The final documentation commit does
not change the tested runtime code.

The floating surface now contains a borderless, larger input and a single
submit arrow. Navigation, bookmark and site controls moved to the browser menu.
Super+Alt+L opens an empty search; Ctrl+L selects the current address. Opening a
URL on startup leaves focus on the page. The visibility state is independent
of whether the window has been mapped, preventing a startup composer from
remaining open over a loaded page.

The real GTK/Xvfb smoke test covers initial page focus, both shortcuts,
Enter and submit-button activation, Escape/outside-click dismissal, single-instance
activation, narrow layout and the existing browser/session/download checks.
Fresh captures: [typed search](docs/composer.png), [empty prompt](docs/composer-empty.png),
[narrow URL editing](docs/composer-narrow.png). These are Ubuntu X11 captures,
not Omarchy/Hyprland acceptance. Live compositor shortcuts, IME, accessibility
and fractional scaling remain untested.

## Initial floating address bar (historical)

Based on v0.0.2 source `dfcbf66d47b528b5a3da2660e6d8880b6d8f84e9`.
The address/navigation controls now live in a centred GTK overlay. The GUI
fixture checks both shortcuts, Enter navigation, Escape and outside-click
dismissal with page focus restored, and single-instance `--focus-address`
without adding a tab. It captures normal and narrow-window layouts.

[Workflow 35930837502](https://github.com/tcballard/nagi/actions/runs/35930837502)
passed for branch source `750d4909ca5c7465b66afe7b5b3199051de9ad8d`, tested
through GitHub's merge with unchanged main `dfcbf66d47b528b5a3da2660e6d8880b6d8f84e9`.
The documentation/screenshots added after this source do not change runtime code.

| Check | Environment | Result |
| --- | --- | --- |
| `cargo fmt --check`, `cargo test --locked` | Ubuntu 24.04, Rust 1.98.1 | Exit 0; all six tests passed |
| `cargo build --locked`, release build | Ubuntu 24.04, system GTK4/WebKitGTK6 | Exit 0 |
| `tests/gui_smoke.py` under Xvfb/dbus, cairo renderer | Real GTK X11/WebKit with local HTTP fixture | Exit 0; both shortcuts, navigation, focus restoration, CLI reuse and existing browser smoke checks passed |
| `makepkg`, package install/version/remove | Arch x86_64 container, ordinary build user | Exit 0 |
| Approved icon pixels, installed GTK lookup, desktop file validation | Ubuntu 24.04 | Exit 0 |

[Normal-window capture](docs/floating-address.png) and
[520px-wide capture](docs/floating-address-narrow.png) were visually inspected:
centred panel, complete controls, no panel clipping. These are real CI captures,
not Omarchy desktop screenshots. SHA-256 respectively:
`2bf8508f916de16ea81b5fdffc1df90515944b587158d6a7adb3b81b21379fae` and
`50c37c16bab0bdb2c32696da3a8daf2ea3824b1556142bc683f3390282ae93c7`.

The initial workflow at `ad4bd29` stopped at a rustfmt discrepancy, corrected
before the passing run above. Local package/toolchain installation was not
available in the current workspace; no local runtime pass is claimed.
Actual Hyprland interception, desktop-wide activation, IME, accessibility with
an active accessibility bus, and fractional scaling remain untested.

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

### Composer capture hashes

- `composer.png`: `a6dc17541ac97abad072e37fe4a23456a669c0cb568960fd389d178887279a69`
- `composer-empty.png`: `d5dadb394e8af945665dc4dca22d3af4de2cfe4a1103c4c05d2eff490ddf6f29`
- `composer-narrow.png`: `37d2c74b90cb9df22763ac8294e9309431d933867320b1a2685f7a53bf558e05`
