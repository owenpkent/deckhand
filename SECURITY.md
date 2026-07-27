# Security policy

## Supported versions

There are no releases and no code yet; the project is at Phase 0,
specification only. Until a first release exists, `main` is the only line,
and security reports against the *design* are welcome: finding a hole in
[docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) now is worth more than
finding it in software later.

## Reporting

Please do not open public issues for vulnerabilities.

- Preferred: GitHub Security Advisories:
  [Report a vulnerability](https://github.com/owenpkent/deckhand/security/advisories/new)
- This is a solo, spare-time project. Reports get read promptly; fixes get
  honest timelines rather than promised ones.

## Scope

In scope, once code exists (and as design critique today):

- The permission-decision path: anything that could produce an `allow`
  without an explicit human click or an explicit, attributed rule.
- The loopback endpoint and its token handling.
- The surface's approval UI: misclick, spoofing, or timing attacks that
  could turn a benign click into an approval.
- Handling of Claude Code settings files (hook install and removal).
- Leakage of session metadata, tool inputs, or the audit log.

Out of scope:

- Claude Code itself, the Claude models, and Anthropic services: report those
  to Anthropic.
- Attacks requiring arbitrary code execution as the same user; see the
  residual-risks section of
  [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md).
- The Codex Micro hardware and Work Louder software.

## One safety note

Deckhand's whole risk budget is spent on a single feature: it can answer
Claude Code's permission prompts. The design rule is that every failure path
returns the decision to the user (`ask`), and no timeout, crash, or default
ever produces `allow`. If you can construct a counterexample to that
sentence, that is exactly the report this file exists for.
