Nagi v0.0.3 gives your agent a supported way to make the browser yours, while Nagi maintains the underlying GTK/WebKit shell.

- Change layouts, shortcuts, toolbar actions and preferences through the CLI or settings panel. Configuration changes are atomic, support revision checks and dry runs, and can be undone.
- Export and apply personal profiles. Start in safe mode to recover from broken customisation.
- Let an agent read and operate explicitly shared tabs through `nagi browser`: page snapshots, screenshots, navigation, clicks, typing and selection. Native per-origin approval, private-tab exclusion, revocation and a visible Stop button keep control with you.
- Install personal Nagi extensions for sidebars, new-tab pages, commands and site hooks. Enable them through native review; changes require fresh approval. A watchdog disables hung extensions.

CLI first. No MCP server required. See [the agent guide](https://github.com/tcballard/nagi/blob/v0.0.3/docs/agent-development.md) for the supported contract and limits.

### Install or upgrade

Close Nagi first. Back up `${XDG_STATE_HOME:-$HOME/.local/state}/nagi` before upgrading.

```sh
curl -fLO https://github.com/tcballard/nagi/releases/download/v0.0.3/nagi-0.0.3-1-x86_64.pkg.tar.zst
curl -fLO https://github.com/tcballard/nagi/releases/download/v0.0.3/ARCH-SHA256SUMS
grep '  ./nagi-0.0.3-1-x86_64.pkg.tar.zst$' ARCH-SHA256SUMS | sha256sum -c -
sudo pacman -U ./nagi-0.0.3-1-x86_64.pkg.tar.zst
nagi
```

For browser control, close the existing instance and launch `nagi --agent-control`. Approve origins and share tabs in the browser. Personal extensions remain disabled until you enable them.

### Verification and limits

Publication is gated on Rust tests, configuration/profile recovery and interrupted-write checks, real GTK/WebKit browser fixtures under Ubuntu/Xvfb, extension revocation/watchdog tests, and an Arch package build/install/remove check. Download checksums accompany the assets.

This remains an early preview. Live Omarchy/Hyprland acceptance, authenticated websites, media, accessibility and sustained daily use still need testing. Personal Nagi extensions are not Chrome/Firefox extensions. A password vault, passkeys, sync and guaranteed DRM playback are not included. The local control socket is a same-user interface, not isolation from hostile processes running as your user.

Upgrades preserve browsing data. Settings migrate to a versioned document on the first change. Before downgrading to v0.0.2, export `nagi config get > settings-v0.0.2.json`, close Nagi, and restore that flat settings file with the older package; keep your backup. Safe mode and config undo help recover preferences but do not undo page actions.
