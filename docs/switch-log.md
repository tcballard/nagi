# Record a browser switch (A2)

Run `nagi log-switch` to answer two keyboard prompts, or supply both arguments
directly: `nagi log-switch "Google Docs" "editing failed"`. Keep the answers short
to finish in a few seconds. Nothing is sent anywhere. The append-only file is
`$XDG_DATA_HOME/nagi/switches.jsonl`, normally
`~/.local/share/nagi/switches.jsonl`, mode 0600 in a mode-0700 directory. Each
JSON line contains a Unix timestamp in seconds, a site or task, the reason,
and the Nagi version. This contains potentially sensitive workflow descriptions;
do not commit or publish the raw file.

To summarize the last week locally, run `nagi log-switch --summary`. Use
`--since 14d` or `--since 24h` to choose a window; `--markdown` produces a
pasteable heading and grouped counts. Keywords are case folded and counted
once per switch; the top ten causes group identical site/task and reason pairs.
The summary names sites and reasons, so review it before sharing it. A later
stats export should include only the switch count by default.

Optional keyboard-only Hyprland binding (edit your own binding file; Nagi does
not alter it):

```lua
o.bind("SUPER + ALT + B", "Log browser switch", "foot --app-id nagi-switch-log -e nagi log-switch")
```

The command needs `foot` (or substitute your installed terminal) and `nagi` on
the session PATH. When the prompt appears, enter the site/task, press Enter,
enter a short reason and press Enter. Nagi does not automatically switch your
browser or monitor other applications.
