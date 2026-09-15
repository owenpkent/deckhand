# Executive summary

Status: **accepted**. This is a derived summary, not an authoritative
source: it is written to be read first and to stay honest, but where it
disagrees with any of the documents it summarises, the linked document wins.

## The problem

Running more than one Claude Code session at a time means running more than
one terminal window, and watching all of them is manual work. Every session
that finishes a turn, hits a permission prompt, or errors out needs to be
noticed, and the only way to notice is to look at it: switch tabs, switch
windows, read the last few lines, decide whether it needs you. Multiply
that by six sessions and the overhead stops being trivial.

That overhead is not evenly distributed. For most people it is an
annoyance. For someone for whom moving between windows, tabs, and keyboard
focus is itself physically expensive, the cost of "just check the other
terminal" is not a few seconds, it is a few seconds every time, many times
an hour, compounding across a working day. A monitoring problem that
everyone tolerates is a bigger problem for people who cannot tolerate it
cheaply. Deckhand exists to move that cost down.

## Prior art

The Codex Micro, a limited-run macropad built by Work Louder in
collaboration with OpenAI, is the direct inspiration for this project and
deserves to be credited plainly rather than treated as a vague influence.
It is a physical device: agent keys bound to ChatGPT chats, command keys for
approve, deny, and similar actions, a dial, a stick, and status colours that
let you read a session's state at a glance instead of reading its text.
What it gets right, and what Deckhand is trying to keep, is the core idea
that managing several agent conversations is a status-board problem as much
as it is a chat problem, and that a small, fixed, physical-feeling control
set can be a better interface for that than a window manager is.

Deckhand is not a Codex Micro clone and is not affiliated with Work Louder
or OpenAI. It borrows the interaction model on purpose and points it at a
different backend, Claude Code, and a different constraint, mouse-only
operation, that the original hardware device did not need to solve because
it was hardware.

## What Deckhand is

Deckhand is a piece of software: an always-on-top, frameless, on-screen
surface, operated with a pointer alone, that lets you see the live status of
every Claude Code session on the machine at a glance, in an ordered list
that grows and shrinks as sessions start and end, and raise any one of them
to the front in the same click that selects it. As of
[ADR-028](DECISIONS.md#adr-028) (2026-09-13), that click is currently the
whole surface: approving a call, denying one, and answering a question stay
Phase 2 or later work and are not currently planned to land on this
surface; a control returning to it needs its own ADR. The status board
half of this exists and watches live sessions; everything that acts on a
session is still design (see the Status section at the end). Deckhand is
independent software, not affiliated with or endorsed by OpenAI, Work
Louder, or Anthropic.

It has two ways of relating to a Claude Code session, called attached mode
and hosted mode, and which one is in use matters more than any other single
fact about how a given session behaves. Attached mode watches a session
started by the user; it gets full status observation and full approve and
deny authority through Claude Code's hook system, but it has no proven way
to put a prompt into that session at the moment a person wants to type one.
Documented channels do exist and they all deliver at a turn boundary rather
than into an idle session, and none of them has been observed working here,
so Deckhand declares the capability false and ships no send. Continue is a
visible, disabled button that states the reason; sending and interrupting
fall back to synthetic keystrokes aimed at the terminal window only if the
user opts in, which is fragile and off by default. Hosted mode starts
sessions itself through the Claude Agent SDK, which gets full control
including sending prompts, at the cost of the normal terminal UI.

Cutting across that is a second axis, the host: what is holding a session's
process, as opposed to who started it. An attached session can sit on a
terminal or inside the VS Code extension, and the difference is real.
Everything hooks provide is identical on both, which covers status, approve,
and deny. What differs is everything that needs a window: inside the
extension there is no window of the session's own, so the synthetic
fallbacks do not exist and Reveal can raise the editor window but not pick
the session's tab inside it. Capabilities therefore attach to a session
rather than to an adapter.

## What is genuinely new

Two things here are not just a port of the Codex Micro's idea to a new
backend.

The first is the status board itself, applied to coding agent sessions
specifically. Chat status boards exist already. A board that reads Claude
Code's own hook events to infer thinking, waiting, done, and error states
for several concurrent coding sessions, and shows that at a glance, is a
narrower and more specific thing.

The second is routing permission gating through the surface rather than
through a terminal prompt. Claude Code already asks for permission before
tools that need it; Deckhand's contribution is not inventing that gate, it
is moving where the answer comes from: a physical-feeling approve or deny
target on an always-on-top surface, rather than a line of text waiting in a
terminal the user may not currently be looking at. For someone for whom
switching focus to that terminal is the expensive part, that relocation is
most of the value.

## How it works

In plain terms, with no jargon: Claude Code can be configured to call a
small program at specific moments, for example when a session starts, when
it is about to use a tool, or when it finishes a turn. Deckhand installs
itself as that small program. Every time one of those moments happens,
Deckhand's local background service hears about it and updates the status
of that session's row in the list. The same channel could carry a
permission answer back the other way, but that direction is Phase 2 or
later work and is not wired up yet, nor currently planned on the surface
(ADR-028); today the channel only reports.

That is the whole mechanism for hooks. Separately, the daemon also holds a
handle on each session's own process and checks it every two seconds; if
the process is gone and no clean exit was reported, the row is removed the
same as if it had exited cleanly, catching a crash or a closed terminal
without needing a hook for it ([ADR-035](DECISIONS.md#adr-035)).

There is no fallback that reads a session's conversation log from disk:
Deckhand never reads transcripts. The same background service also asks
Claude Code directly, on a fifteen-second timer, what each session is
doing. Hooks are trusted first, but if hooks and that periodic check
disagree twice in a row with no hook heard in between, the periodic check
wins and the row is corrected within about thirty seconds instead of
staying wrong until the next hook arrives ([ADR-036](DECISIONS.md#adr-036)).

## What is uncertain

This section is here because leaving it out would be dishonest.

- Whether status inferred from hook events is reliable enough to trust at a
  glance is not yet proven. Phase 1 exists specifically to test this before
  anything is built on top of it.
- Whether hook call overhead stays negligible as several sessions report
  concurrently is unmeasured, and the list is unbounded rather than capped
  at a fixed count. If overhead is not negligible, the architecture needs
  to account for that honestly rather than hope it away.
- Whether attached mode is genuinely useful without the ability to inject a
  prompt is an open question, not a settled one. It may turn out that
  attached mode is mostly a status board with limited action, and hosted
  mode is where the real control lives, in which case the documentation
  should say that plainly rather than imply parity between the two modes.
- Whether a genuinely failed turn (as opposed to a crashed process, which
  the daemon now detects without hooks, see "How it works") surfaces as a
  distinct hook event at all is not yet confirmed; a documented event
  exists but has not been seen firing.
- macOS and Linux support is intended but unproven. Windows 11 is the only
  platform anything here has been reasoned through concretely.

## Status and what happens next

Deckhand is at Phase 1: observation. The specification is complete, and
the first real code exists: a daemon and row surface in one desktop
application plus the small program Claude Code's hooks call, watching
live sessions and painting their status. Nothing in it can approve,
deny, or send anything yet; that authority arrives in Phase 2, after
the watching has earned trust. One Phase 0 item remains open alongside:
validating the last few hook events against a live install. See `ROADMAP.md` for the phase breakdown and `TODO.md` for what is
currently open, including the specific validation work, against a real
Claude Code install rather than just its documentation, that Phase 0 still
owes before Phase 1 can start on solid ground.
