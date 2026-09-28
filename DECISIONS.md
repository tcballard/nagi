# Nagi next-phase decisions

## 2026-09-24 — validation before expansion

Source: Tom's Next-Phase Handoff, 24 September 2026.
Status: Tom confirmed the G3/G4 hold-and-retest recommendations and keeping all
52 compatibility sites on 2026-09-24. All gates remain NOT RUN; no gate has passed.

Feature freeze: no new user-facing features, surfaces or extension capabilities.
Only A1–A7 and compatibility fixes explicitly approved by Tom are exceptions.
One PR per task; include its ID, definition-of-done checklist and evidence for
individual items. Pending human checks remain unchecked. All measurements are
local; export requires an explicit user command. Never commit credentials,
authenticated profiles or private screenshots. Review baseline artifacts before
committing them; redact sensitive content and record redactions.

## Thresholds (handoff plus confirmed G3/G4 dispositions)

| Gate | Pass | Middle | Fail / action |
|---|---|---|---|
| G1, end of week 2 | Pass rate >=90% and zero blocked critical workflows | 75–90%, or blocked workflows limited to DRM media: position as an agent-shaped work browser, retaining Chromium for streaming | <75%, or any blocked critical work workflow: stop feature work and evaluate pivot |
| G2, after A3 and T4 | All fixtures blocked and zero successful manual attacks left unfixed | None | No demo or testers until fixed |
| G3, after A5 and T5 | Nagi-only chrome tasks >=80% first-try success; feasible-in-both Nagi success >= baseline +15 percentage points | Nagi-only >=80%, without a clear feasible-in-both win: pitch chrome control only | Nagi-only <60%: fix API before promotion; 60–<80%: hold promotion, improve API and retest (confirmed) |
| G4, cohort day 30 | >=1/3 retained as default AND crashes <1 per 100 active hours | 1/5–1/3 retained: another 30-day cycle fixing top three switch causes | <1/5 retained: pivot evaluation; >=1/3 retained with crashes >=1 per 100 hours: hold, fix reliability and retest (confirmed) |

A missing export counts as not retained. Freeze the enrolled cohort denominator
before collecting retention outcomes. Zero active hours cannot establish a crash
rate; report unavailable, never zero.

A1 pass rate = pass / (total - blocked-auth); apply the identical formula to
critical sites for critical pass rate. A zero denominator is unavailable.
Blocked workflows are critical sites with status fail. Report blocked-auth
counts prominently; excluded authenticated sites are missing evidence, not
proof that those workflows work.

Pivot evaluation: at most two weeks assessing Firefox as a target for the API,
schema, approval model and harness; record go/no-go. Do not start a Chromium fork.

## Confirmed decisions — 2026-09-24

Tom's response: "Yeah okay I agree, and keep all 52".

- G3: 60% through <80% Nagi-only first-try success means hold promotion,
  improve the API and retest.
- G4: passing retention with failing reliability means hold, fix reliability
  and retest; retention alone cannot pass the gate.
- A1/T1: retain all 52 Appendix 1 entries. This replaces the original 50-site
  count in the A1 scope and T1 acceptance criterion. T1 still requires review
  of URLs, critical flags and one-time logins. This does not change A5's
  separate requirement for 50 evaluation tasks.

## Remaining inputs (do not block A1/A2 implementation)

- G1 adjudication: resolve failure precedence over the DRM middle condition
  and how missing authenticated critical-workflow evidence prevents a premature
  decision. Proposed: critical-work failure or <75% overrides DRM; >=90% with
  no blocked workflows passes; remaining >=75% results are middle.
- G3: explicitly define "no clear win" as below the +15 percentage-point
  threshold before evaluation data arrives.
- G4: clarify the middle-band crash requirement before cohort results arrive.
  Exact 1/3 retention meets the retention component of pass, but the strict
  crash requirement must also hold.
- T1: supply bank, Mastodon, Atlassian tenant and local development-server URLs,
  plus final critical flags. No guessed bank or tenant.
- A5/T5: specify the agent CLI/model and approve the 50 task candidates before
  measured runs. Human approvals required by A3 must be reflected honestly in
  the first-try/no-human-help metric; do not bypass consent for evaluation.

## Execution and dependencies

- A1 compatibility and A2 switch logging in parallel; Tom completes T1 and starts
  T2/T5. A1 remains incomplete until a real baseline and repeatability run exist.
- A3 security hardening next; Tom's week-two T3 records G1.
- T4 follows A3 merge, includes an hour and three novel attacks, then records G2.
- A4 and A5 follow the recorded gates in the handoff; T5 grades lead to G3.
- A6 and A7 follow; T6 must verify onboarding on a clean account.
- Demo requires G1 and G2 passed. Recruitment also requires A3/A6/A7 and the
  preceding gates completed. No automated test can substitute for these sign-offs.
- T7/T8/T9 lead to G4; public pitch and maintainer approach follow the handoff.

## Existing convention mapping

Source: README.md, AGENTS.md and docs/agent-development.md on main, inspected
2026-09-24. These are documented existing capabilities, not new verification.

- A1: keep GTK4/system WebKitGTK; do not substitute Playwright's WebKit for Nagi.
  Existing browser control intentionally has no arbitrary evaluation API. Any
  test instrumentation must stay test-only and preserve consent boundaries.
- A2: use the existing `nagi` CLI namespace and documented XDG conventions.
- A3: strengthen existing native extension consent, grants, Stop and browser
  sessions; existing documentation explicitly does not isolate hostile same-user
  processes. A new session does not erase an agent's knowledge of malicious text.
  Fixture results must distinguish policy enforcement from model susceptibility.
- A4: extend `nagi config schema`, `nagi extension schema`, versioned profiles,
  transactions and migration handling instead of creating parallel schemas.
- A5: reuse configuration inspect, revision checks and undo; never invent scores.
- A6: add local counters only, no browsing content in default exports.
- A7: reuse packaging, `nagi --safe-mode`, recovery and uninstall instructions.

## Evidence and current blockers

This initial decision document changes no runtime code. The current session has
GitHub repository access but no shell/build/desktop execution tool. No Rust,
GTK, packaging, compatibility, repeatability or security tests were run here.
The v0.0.3 handoff's passing tests are prior evidence, not a new result.
Real Omarchy runs, one-time human logins and Tom's gate decisions remain required.
