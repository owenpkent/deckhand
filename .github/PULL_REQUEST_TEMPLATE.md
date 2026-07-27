## Description

<!-- What does this change do, and why? -->

## Related issue

<!-- Link the issue this addresses, if any. "Closes #123" auto-closes it on merge. -->

## Type of change

- [ ] Specification change (a `docs/` file, with no code impact yet)
- [ ] New feature
- [ ] Bug fix
- [ ] Breaking change (changes existing behaviour or a documented contract)
- [ ] Documentation only (README, CONTRIBUTING, and similar, not a `docs/`
      specification file)
- [ ] Chore (tooling, dependencies, repository maintenance)
- [ ] Refactor (no behaviour change)

## Test plan

<!--
How was this verified? For a specification change, this can mean "reviewed
against docs/DECISIONS.md for conflicts" or "checked every cross-reference
resolves". For code, once there is code, list the actual steps or commands.
-->

## Accessibility check

- [ ] Operable with a pointer alone, start to finish, no keyboard fallback
      required
- [ ] Any new hit target meets the documented minimum size
      (`docs/UI_SPEC.md`)
- [ ] Status is not conveyed by colour alone (a glyph or label goes with it)
- [ ] No new interaction requires a sustained hold to complete
- [ ] Tested at the smallest supported surface size, not only a comfortable
      default window size

If a box does not apply to this change, check it only once you have
confirmed it still holds; otherwise leave it unchecked and say why below.

## Sync checklist

Deckhand's specification is spread across several documents that need to
move together. The change propagation table in
[`docs/WORKFLOW.md`](../docs/WORKFLOW.md) is the authority on what a given
kind of change needs to touch. At minimum:

- [ ] `docs/CONTROL_MAPPING.md` updated, if a control was added or changed
- [ ] `docs/ADAPTER_PROTOCOL.md` updated, if an adapter capability changed
- [ ] `docs/SECURITY_MODEL.md` updated, if the trust boundary moved
- [ ] `CHANGELOG.md` updated
- [ ] `TODO.md` / `ROADMAP.md` updated, if this closes or opens a tracked
      item
