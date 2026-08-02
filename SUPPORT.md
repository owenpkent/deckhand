# Support

Deckhand is a personal project maintained in spare time. There is no SLA, but
every issue gets read.

## Before you post

- Check [README.md](README.md) for what Deckhand does and does not do yet.
- Check [docs/CLAUDE_CODE_ADAPTER.md](docs/CLAUDE_CODE_ADAPTER.md) if the
  problem is about status colours or actions not landing. Most surprises live
  there, because parts of that integration lean on interfaces Claude Code does
  not promise to keep stable.
- Check [ROADMAP.md](ROADMAP.md) before asking whether something is planned.
- Search [existing issues](https://github.com/owenpkent/deckhand/issues?q=is%3Aissue).

## You want to

| You want to | Go to |
| --- | --- |
| Report something broken | [Bug report](https://github.com/owenpkent/deckhand/issues/new?template=bug_report.yml) |
| Ask for a capability | [Feature request](https://github.com/owenpkent/deckhand/issues/new?template=feature_request.yml) |
| Say where it is hard to operate | [Accessibility feedback](https://github.com/owenpkent/deckhand/issues/new?template=accessibility_feedback.yml) |
| Ask for support for another agent runtime | [Adapter request](https://github.com/owenpkent/deckhand/issues/new?template=adapter_request.yml) |
| Report a security problem | [SECURITY.md](SECURITY.md), **not a public issue** |
| Ask a question, share a setup, show a layout | [Discussions](https://github.com/owenpkent/deckhand/discussions) |
| Help build it | [CONTRIBUTING.md](CONTRIBUTING.md) |

## What "supported" means right now

Deckhand is at **Phase 1: observation**. There is no installer yet; the
board builds from source on Windows (`python scripts/run.py`) and only
watches, never acts. Issues about the design, the control mapping, the
adapter contract, and the Phase 1 board are in scope and welcome.

Windows 11 is the reference platform. macOS and Linux are intended, not proven.

## Redaction reminder

Claude Code session transcripts contain your prompts and your source code.
Deckhand reads them locally and never sends them anywhere, but **you** might
when you paste a log into an issue. Please check before you post.
