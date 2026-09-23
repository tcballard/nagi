# Verification

Target: Omarchy 4 / Hyprland. No installed Omarchy version has been exercised yet.
This is a release candidate, not a claim of complete browser feature parity.

## Reproduced locally

Ubuntu 24.04 x86_64; Rust 1.98.1; GTK 4.14.5; WebKitGTK 2.52.3.

- `cargo check --locked`: passes.
- `cargo build`: passes.
- `cargo test --locked`: six tests pass, covering URL/scheme handling, origin
  boundaries, bounded/deduplicated history, filename/text escaping, atomic
  final-save/reopen, preservation of corrupt/newer data, and theme validation.

## Environment limitation

The local desktop test could not start: dbus-daemon was denied socket creation
by this workspace. This is not a successful GTK/WebKit runtime test.

## Automated desktop checks

The GitHub Actions workflow builds the actual browser and runs
`tests/gui_smoke.py` against an HTTP fixture under Xvfb. Its artifact contains
the result, runtime log, screenshots and (after all checks pass) release binary.
Inspect the workflow result for the delivered commit; merely including the
workflow does not establish that it passed.

## Still requires real Omarchy acceptance

Wayland / Hyprland launcher identity, portals, fractional scaling, IME,
clipboard, multi-monitor behaviour, live system theme switching, authenticated
website compatibility, camera/audio permissions, video playback and package
install/upgrade/removal. The Arch recipe is supplied but has not been built in
an Arch clean chroot. aarch64 is declared by the recipe but not tested here.
