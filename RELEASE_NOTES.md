Nagi v0.0.2 brings the approved moon-and-horizon icon into the browser and desktop launcher.

- Native small-size artwork and nine PNG exports from 16 to 512px.
- A symbolic icon that follows panel text colour.
- Updated tab, welcome-page, window and reading-view icons.
- Complete hicolor installation and cleanup for Arch packages and user-local installs.

Install or upgrade on x86_64 Omarchy / Arch:

```sh
curl -fLO https://github.com/tcballard/nagi/releases/download/v0.0.2/nagi-0.0.2-1-x86_64.pkg.tar.zst
sudo pacman -U ./nagi-0.0.2-1-x86_64.pkg.tar.zst
nagi
```

The release workflow gates publication on icon pixel checks, GTK icon lookup, actual browser workflows under Ubuntu/Xvfb, and an Arch package build/install/remove check. SHA-256 files accompany the downloads.

The icon uses the static Tokyo Night palette. Real Omarchy/Hyprland acceptance remains outstanding. Browsing data is preserved on upgrade. This is still an early preview; extensions, a password vault, passkeys, sync and guaranteed DRM playback are not included.
