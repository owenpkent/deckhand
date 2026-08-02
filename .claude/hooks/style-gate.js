#!/usr/bin/env node
// Deckhand PreToolUse style gate. Blocks, before anything is written, the
// two things CLAUDE.md and .github/workflows/docs.yml both forbid: em or en
// dashes in authored markdown, and AI attribution in commit messages.
// Uses node because jq is not installed on this machine.
//
// This is a pre-flight, not the enforcement point. scripts/check-docs.ps1 is
// authoritative and is what CI runs; this gate exists to catch a violation
// one second after it is typed instead of ten minutes later in a CI log.
// It sees Write and Edit only, so markdown written through a Bash heredoc
// passes it untouched and is caught by CI instead. Keep the thresholds here
// equal to the script's so the two never disagree about the same line.
//
// Fail-open contract: this gate must never brick editing. Any parse error,
// unexpected payload shape, or internal fault exits 0 with no output, which
// leaves the tool call to the normal permission flow. Only a positive,
// confidently detected violation produces a deny.

"use strict";

// Nothing this file can throw is worth blocking an edit over.
process.on("uncaughtException", () => process.exit(0));
process.on("unhandledRejection", () => process.exit(0));

// Written as escapes on purpose: the literal characters are banned in this
// repo, including inside the gate that bans them.
const DASHES = /[\u2014\u2013]/g;
// Same exemptions as the CI style job: inherited third-party texts. Anchored
// to a path boundary because check-docs.ps1 compares the leaf filename, and
// a bare suffix match would exempt something like OUR_CODE_OF_CONDUCT.md here
// that CI would still fail.
const EXEMPT = /(^|\/)(CODE_OF_CONDUCT|CONSTELLATION_INTEGRATION_GUIDE)\.md$/i;
const ATTRIBUTION =
  /Co-Authored-By:\s*Claude|Generated with \[?Claude|noreply@anthropic\.com/i;

function emit(payload) {
  try {
    process.stdout.write(JSON.stringify(payload));
  } catch (e) {
    // Fall through to exit 0: an unwritable stdout is not a violation.
  }
  process.exit(0);
}

function deny(reason) {
  emit({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: reason
    }
  });
}

function checkWrite(ti) {
  const p = String(ti.file_path || "").replace(/\\/g, "/");
  if (!/\.md$/i.test(p)) return process.exit(0);
  if (EXEMPT.test(p)) return process.exit(0);

  const chunks = [ti.content, ti.new_string];
  if (Array.isArray(ti.edits)) {
    for (const e of ti.edits) {
      if (e && typeof e === "object") chunks.push(e.new_string);
    }
  }
  const text = chunks.filter((c) => typeof c === "string").join("\n");
  if (!text) return process.exit(0);

  const hits = text.match(DASHES);
  if (hits) {
    return deny(
      "House style: " + hits.length + " em or en dash(es) in " + p +
      ". The docs CI style job fails on these. Rewrite with a comma, " +
      "colon, parentheses, or a period, then retry the edit."
    );
  }

  // Wrap width is a warning, never a block: tables and URLs legitimately
  // run long, and roughly 45 lines in the repo already exceed 80 columns.
  // The threshold and the two exclusions match gate 5 of check-docs.ps1
  // exactly. A gate that warns at a different column than CI reports is
  // worse than no gate, because it teaches you to ignore both.
  const long = text.split("\n").filter(
    (l) => l.length > 80 && !/^\s*\|/.test(l) && !/https?:\/\//.test(l)
  );
  if (long.length) {
    return emit({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        additionalContext:
          long.length + " prose line(s) exceed the ~80 column wrap in " +
          p + ". Not blocked, but rewrap before pushing."
      }
    });
  }
  return process.exit(0);
}

function checkBash(ti) {
  const cmd = String(ti.command || "");
  if (!/\bgit\s+commit\b/.test(cmd)) return process.exit(0);
  if (ATTRIBUTION.test(cmd)) {
    return deny(
      "AI attribution is a firm rule violation in this repo (CLAUDE.md). " +
      "Remove the Co-Authored-By, Generated with, or anthropic noreply " +
      "trailer and recommit."
    );
  }
  return process.exit(0);
}

let raw = "";
process.stdin.setEncoding("utf8");
process.stdin.on("error", () => process.exit(0));
process.stdin.on("data", (c) => {
  raw += c;
});
process.stdin.on("end", () => {
  // Field-capture tap for the pre-Phase-1 payload spike (TODO.md): append
  // every raw event to gitignored _scratch/ before any gating, so each
  // fire enumerates real payload fields instead of documented ones.
  // Fail-open like everything else in this file.
  try {
    const fs = require("fs");
    const path = require("path");
    const dir = path.join(__dirname, "..", "..", "_scratch");
    fs.mkdirSync(dir, { recursive: true });
    fs.appendFileSync(
      path.join(dir, "hook-capture.jsonl"),
      raw.trim() + "\n"
    );
  } catch (e) {
    // Capture is best-effort; gating continues regardless.
  }

  let d;
  try {
    d = JSON.parse(raw);
  } catch (e) {
    return process.exit(0);
  }
  if (!d || typeof d !== "object") return process.exit(0);

  const tool = typeof d.tool_name === "string" ? d.tool_name : "";
  const ti =
    d.tool_input && typeof d.tool_input === "object" ? d.tool_input : {};

  if (tool === "Write" || tool === "Edit") return checkWrite(ti);
  if (tool === "Bash") return checkBash(ti);
  return process.exit(0);
});
