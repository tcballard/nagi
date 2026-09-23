Nagi is a quiet native browser for Omarchy, built with Rust, GTK4 and WebKitGTK.

This first versioned preview includes tabs, session restoration, private browsing,
bookmarks, local history, downloads, reading view, element hiding and basic
tracker blocking. It follows Omarchy's theme and provides a desktop launcher.

**Install on Omarchy / Arch:** download `nagi-0.0.1-1-x86_64.pkg.tar.zst` and run:

```sh
sudo pacman -U ./nagi-0.0.1-1-x86_64.pkg.tar.zst
nagi
```

The package build and install/removal checks run in an Arch container. Desktop
smoke checks run the sandboxed WebKit browser on Ubuntu with Xvfb and local test
pages. Real Omarchy/Hyprland desktop acceptance is still outstanding.

This is not yet a Chrome/Firefox replacement: extensions, a password vault,
passkeys, sync and guaranteed DRM playback are not included. Keep system
WebKitGTK updated through pacman. The app does not change your default browser.
