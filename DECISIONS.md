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

## 2026-09-28 — direction after the Cua comparison

Tom confirmed this direction after reviewing Nagi against Cua Driver.
Prioritise reversible, schema-driven browser personalisation and controlled
delegation. Generic page automation alone is not sufficient justification for
asking someone to switch browsers. Demonstrate value through measured tasks.

PRs #8–#11 are merged. Their merge does not complete A1, A2 or A3 and does
not pass G1 or G2. The 52-site baseline, live switch-log timing and hostile-page
security acceptance remain outstanding.

### Next implementation: finish A3's authority boundary

The next A3 PR must define the actual agent execution environment and enforce
the following contract before claiming that human approval protects settings:

- [ ] The browsing agent can submit a bounded settings proposal with a reason,
  but cannot apply or approve that proposal itself.
- [ ] The human preview shows the exact keys and old/new values. Approval binds
  the proposal, settings revision and relevant extension digest; stale or
  changed proposals require a fresh review.
- [ ] All persistent settings paths share the same enforcement boundary,
  including CLI, profiles and extension commands. Direct filesystem access
  from the isolated browsing agent cannot bypass it.
- [ ] The agent cannot drive the approval surface through accessibility,
  synthetic pointer/keyboard input or another desktop-control tool. Define
  and test a trusted human approval channel; a native GTK dialog alone does
  not establish human provenance.
- [ ] Approved changes apply atomically, update the running browser, retain
  undo, and create a durable audit record with the proposal reason and outcome.
  Keep browsing content and secrets out of default records.
- [ ] Stop/revoke invalidates pending proposals and session capabilities.
  Replayed, expired, changed or cross-session approvals fail without mutation.
- [ ] Safe mode and recovery remain usable without granting agent authority.

Start with a concrete threat model and smallest enforceable isolation design.
Document which shell, filesystem, desktop and IPC capabilities are removed.
An unrestricted process running as the desktop user remains outside the
claimed boundary; do not describe a same-user socket or advisory policy as
isolation. Preserve legitimate owner configuration and recovery paths.

### Verification requirements for A3 and subsequent control changes

Adopt independent observation of actual effects:

- [ ] Hostile-page fixtures attempt settings/extension writes, approval
  activation, CLI/filesystem bypass, replay and stale-proposal substitution.
- [ ] Read application-owned state after allowed actions. A successful tool
  response or dispatch acknowledgement alone is insufficient.
- [ ] For claimed background operations, verify foreground focus, physical
  cursor and foreground input state alongside the target effect.
- [ ] Denied actions return a specific refusal and leave protected state
  unchanged. Do not silently foreground a window or widen an input route.
- [ ] Retain source revision, environment, fixture state and redacted evidence
  for each required case. Missing or skipped cases remain unproven.
- [ ] Complete T4 on real Omarchy, including the existing hour and three novel
  attacks, before recording G2. CI fixtures cannot replace that sign-off.

### A5/T5 comparison baseline

Include Cua controlling an established browser as the proposed comparator.
Preflight its exact browser, compositor, driver and input route before freezing
the comparison: current Cua platform documentation does not establish universal
Hyprland or WebKitGTK typed-browser support. Record an unsupported baseline
route as unavailable, never as a successful Nagi comparison.

Retain the existing 50-task approval and G3 thresholds. The task set must cover
ordinary page work and browser personalisation. Use the same model, task
instructions, initial data and time budget where feasible. Report first-try
success, elapsed time, actions, human interventions and recovery/undo outcomes.
Separate tasks feasible in both products from Nagi-only configuration tasks.
Count required human approvals honestly; successful personalisation must be
observed in the live UI and persisted state, then verified through undo.

### Cua interoperability: later decision

After A3/G2 and the comparison evidence, evaluate a narrow adapter for Nagi's
existing capabilities. It must retain Nagi's grants, private-tab exclusion,
reference lifetimes, Stop/revoke and human approval boundary. Any fallback to
desktop control must not let the agent approve its own request. An adapter
must not introduce unrestricted evaluation, shell access or a parallel settings
store. This is a future evaluation, not an implemented integration or an
exception to the feature freeze.

References reviewed on 2026-09-28:
- https://cua.ai/docs/concepts/browser-targeting-and-background-delivery
- https://cua.ai/docs/concepts/how-permission-policies-work
- https://cua.ai/docs/concepts/how-cua-driver-is-validated
- https://cua.ai/docs/reference/cua-driver/platform-support
