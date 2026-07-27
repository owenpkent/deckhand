# Workflow

This document exists to stop the specification from drifting against
itself. Deckhand's design is described from several different angles across
several files. Nothing here overrides those files; this document only says
which file wins when two of them seem to disagree, and what else needs to
change when one of them does.

---

## 1. Source of truth map

| Fact | Authoritative file |
|---|---|
| Control behaviour | `docs/CONTROL_MAPPING.md` |
| Visual and interaction spec | `docs/UI_SPEC.md` |
| Adapter contract | `docs/ADAPTER_PROTOCOL.md` |
| Claude Code specifics | `docs/CLAUDE_CODE_ADAPTER.md` |
| Trust boundary and permission model | `docs/SECURITY_MODEL.md` |
| Decisions and their reasons | `docs/DECISIONS.md` |
| Plan and phasing | `ROADMAP.md` |
| Open work | `TODO.md` |

If two documents disagree about a fact, the file in this table wins for
that fact, and the other document should be corrected to match, not the
other way round. If neither document is a clear authority for a given fact,
that is itself worth raising as an issue: the map above is probably missing
a row.

---

## 2. Change-propagation table

When you make one of these kinds of change, update the marked files in the
same pull request. "Yes" means update it or explicitly re-review it; a
blank cell means it is not typically affected, not that it never could be.

Columns: **CM** `docs/CONTROL_MAPPING.md`, **UI** `docs/UI_SPEC.md`,
**AP** `docs/ADAPTER_PROTOCOL.md`, **CCA** `docs/CLAUDE_CODE_ADAPTER.md`,
**SM** `docs/SECURITY_MODEL.md`, **A11Y** `docs/ACCESSIBILITY.md`,
**DEC** `docs/DECISIONS.md`, **CHG** `CHANGELOG.md`.

| Kind of change | CM | UI | AP | CCA | SM | A11Y | DEC | CHG |
|---|---|---|---|---|---|---|---|---|
| Add or change a control | Yes | Yes | Yes | | | Yes | | Yes |
| Change a status colour or state | Yes | Yes | Yes | Yes | | Yes | | Yes |
| Add an adapter capability | | | Yes | Yes | Yes | | Yes | Yes |
| Change how a permission decision is made | | | Yes | Yes | Yes | | Yes | Yes |
| Change a default | | Yes | | | | | Yes | Yes |
| Change the minimum hit target | | Yes | | | | Yes | | Yes |
| Add a dependency | | | | | Yes | | Yes | Yes |
| Change the stack | | | | | Yes | | Yes | Yes |

`TODO.md` and `ROADMAP.md` are not columns above because they are handled
separately: update them whenever the change closes or opens a tracked item,
regardless of which row this table matches.

This table covers the cases known about now. If you make a kind of change
that is not listed, use judgement, then add a row so the next person does
not have to guess.

---

## 3. Generated versus authored

Nothing in this repository is generated yet; every file here is hand
authored. The rule for when that changes: generated artefacts that other
people need in order to use or build the project are committed, but build
trees, package caches, and anything reproducible purely from a clean
checkout plus a documented command are not. When code generation is
introduced, this section should be updated to say specifically what is
generated, by what command, and where it is checked in.

---

## 4. Definition of done for a change

A change, documentation now and code later, is done when:

- It is internally consistent with every file listed in the source of
  truth map above, or those files have been updated alongside it.
- Every cross-reference it adds or touches actually resolves to a real
  file and section.
- It follows the doc style rules: no em dashes or en dashes, roughly 80
  column wrap, plain and honest tone, nothing claimed to work that does
  not.
- The change-propagation table has been checked, and the files it points
  to have either been updated or confirmed not to need it.
- `CHANGELOG.md` reflects it, under `[Unreleased]`, if it is the kind of
  change a future reader would want to know about.
- For code, once there is code: it is tested, and the test plan is written
  in the pull request, not only in the author's head.

---

## 5. When to push

Push at coherent boundaries, not mid-iteration. A coherent boundary means:
the documents affected by the change agree with each other, every link in
what you touched resolves, and the change could be read on its own, by
someone with no other context, and make sense.

Do not push a change that leaves two documents contradicting each other,
even temporarily, even if you plan to fix the second document in the next
commit. Land them together, or not at all.
