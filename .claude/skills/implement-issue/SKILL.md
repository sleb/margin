---
name: implement-issue
description: Implement a GitHub issue that has a design and a sliced execution plan by delegating each slice, in order, to a sub-agent, then running a comprehensive code review sub-agent and opening a PR. Use whenever the user says "implement issue #N", "work through GH issue N", "build this plan with sub-agents", or points at an issue with slices/steps and wants it taken through to a PR, even if they don't mention sub-agents.
---

# Implement an issue slice by slice

You are the orchestrator. Sub-agents write the code; you keep the plan coherent, catch drift early, and only bother the user for decisions that are really theirs. Sub-agents start cold and cannot see this conversation, so every prompt you write must be self-contained.

## 1. Prepare
- Read the issue and its comments: `gh issue view N --comments`. Identify the slices/steps, the tests each slice calls for, out-of-scope items, and known risks.
- Branch off `main` (e.g. `git checkout -b <short-topic>`). One branch and one PR for the whole issue, one commit per slice, unless the issue says otherwise.
- Find the project's verification commands (README, CLAUDE.md, CI config): tests, linter, formatter, typecheck.
- Note any assets the plan says already exist but aren't in the repo (search scratchpad/earlier sessions) and pass their paths to the relevant slice.

## 2. Run the slices, strictly one at a time
Never run slices concurrently: later slices depend on the code and on the adjudications from earlier ones.

Each sub-agent prompt should contain:
- Repo path, branch, and "slices X..Y are already committed".
- "Implement ONLY slice K" plus the explicit list of things belonging to later slices that it must not touch. Scope creep makes review and rollback harder.
- Pointer to the issue (`gh issue view N`) and the slice's tests.
- Strict TDD: write the failing tests first, see them red, implement minimally, then refactor. Ask it to report if a test could not go red.
- Run the project's tests, linter, formatter check and typecheck; no new warnings.
- Commit locally as one commit with the attribution trailer required by the session's git instructions; do not push.
- Report back: what changed, results, deviations from the plan, issues or questions needing design/plan changes, and what it could not verify (manual/UI/OS behavior). Honest "not verified" beats a silent assumption.
- Any adjudications from earlier slices that affect it.

Choose a sub-agent type that matches the language/domain (e.g. a language specialist) when one is available.

## 3. Check each slice before moving on
After each report, spend a minute, not ten: `git status`, `git log --stat -1`, skim the key files, and re-run the cheap checks if something looks off. Stale editor diagnostics are not evidence; run the tools.

Then adjudicate:
- **Minor deviations you can decide** (a plan assumption that doesn't hold in a test harness, a trait that grows incrementally to avoid dead code, a wrong commit trailer): accept or fix it yourself, and carry the decision into the next sub-agent's prompt. Record it for the PR description.
- **Real design questions** (changes user-visible behavior, contradicts an explicit decision recorded in the issue, needs a new dependency or scope the user ruled out): stop and ask the user. Everything else, decide and keep going. Re-litigating settled decisions wastes the user's time.
- Do not let a sub-agent fix a risk the plan deferred unless it can be verified; unverifiable "fixes" go in the PR as known risks.

## 4. Final review
When all slices are committed, dispatch one read-only code-review sub-agent over `git diff main...HEAD` against the issue. Ask for: correctness vs. the design/acceptance criteria, idioms, test quality (vacuous or weak assertions), platform/cfg gating, error handling, docs accuracy, script robustness, with findings ranked and must-fix separated from nits. Tell it which risks are already accepted so they aren't re-flagged.

Reviewer agent types often have no shell. Run the tests/linter yourself and say so in the PR rather than trusting a review that didn't run them. Fix trivial nits yourself in a small follow-up commit; for substantive findings, send a fix task to a sub-agent or ask the user. Mention unfixed nits in the PR.

## 5. Open the PR
- Re-run the full verification, push the branch with upstream tracking, then `gh pr create`.
- Body: `Closes #N`; what each slice delivered; **deviations from the plan** and why; **not verified** items that need a human (manual checklist from the issue, platform behavior, known risks); a short review summary and open nits; the PR attribution line the session instructs.
- Check for a PR template first (`.github/pull_request_template.md`).

## 6. Subscribe
If a PR subscription/watch tool is available, subscribe to the PR. If not (check with ToolSearch), say plainly that it wasn't possible and offer polling via `/loop` or `/schedule`. Then give the user the PR link and a short summary of what remains for them.
