---
name: sync-check
description: Walk the Deckhand change-propagation table for a pending
  specification change and report which docs still need updating. Use
  whenever a control, status colour, adapter capability, permission
  decision, default, hit target, dependency, timing, hook event, state
  machine, or the stack changed, or before opening a PR that touches docs/.
---

# Sync check

Deckhand's spec is spread across files that must move together. This walks
the table so nothing lands half-updated.

## Steps

1. `git diff --stat main...HEAD` and `git status --short` to get the
   changed set. If nothing is staged or committed, ask which change to
   check rather than guessing.
2. Classify the change against the rows of `docs/WORKFLOW.md` section 2.
   A change can match more than one row. If it matches none, say so and
   propose the new row rather than inventing an ad hoc rule.
3. For every "Yes" cell in the matched rows, check whether that file is in
   the changed set. The nine columns, in table order:

   | Column | File |
   | --- | --- |
   | CM | `docs/CONTROL_MAPPING.md` |
   | UI | `docs/UI_SPEC.md` |
   | AP | `docs/ADAPTER_PROTOCOL.md` |
   | CCA | `docs/CLAUDE_CODE_ADAPTER.md` |
   | ARCH | `docs/ARCHITECTURE.md` |
   | SM | `docs/SECURITY_MODEL.md` |
   | A11Y | `docs/ACCESSIBILITY.md` |
   | DEC | `docs/DECISIONS.md` |
   | CHG | `CHANGELOG.md` |

   Read the table from the file each time. It has grown before and the
   column set is not frozen. CHG is "Yes" in every row today, so a docs
   change with no changelog entry is always a finding.
4. Separately check `TODO.md` and `ROADMAP.md`: they are not columns, but
   update them if the change opens or closes a tracked item. Also check
   `docs/EXECUTIVE_SUMMARY.md`, which is derived and never authoritative
   but drifts first when a frozen concept moves.
5. Guard rails, report a failure if any is broken:
   - ADR-006: the approval path fails to `ask`, never to `allow`. Any
     touch here must update `docs/SECURITY_MODEL.md` in the same change.
   - ADR-008: the six status colours and meanings are frozen. `unknown`
     is the only Deckhand addition. No new states, no repurposed colours.
   - `docs/ACCESSIBILITY.md` rules are requirements: no required holds,
     drags, double-clicks, hovers, or keyboard; 44 px minimum targets.
   - Claude Code integration facts carry a partial verification stamp
     only. Four things are observed against 2.1.220; everything else is
     `documented` or `unverified` and keeps its hedge unless the stamp in
     `docs/CLAUDE_CODE_ADAPTER.md` was upgraded in this same change.
   - Status lines are never upgraded without the evidence that justifies it.
   - ADRs are append-only. A superseded decision gets a new entry and a
     marker, never an edited one.
6. Run `powershell -NoProfile -File scripts/check-docs.ps1 -All` (no
   `pwsh` on this machine; CI uses `pwsh`) and confirm it
   exits 0. That script is where the dash, status line, ADR numbering, and
   Constellation rules live; do not restate them here.
7. Verify every relative link and anchor the change added resolves to a
   real file and heading. `lychee` is not installed locally, so this is an
   approximation of the CI link job, not the same check.

## Output

A table of file, required (yes/no), touched (yes/no), verdict. Then a short
list of concrete edits still owed. Do not make the edits unless asked.
