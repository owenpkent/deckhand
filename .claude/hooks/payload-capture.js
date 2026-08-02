#!/usr/bin/env node
// Deckhand payload capture for the pre-Phase-1 hook validation spike
// (TODO.md). Appends every hook event payload, raw and unmodified, to
// gitignored _scratch/hook-capture.jsonl, one JSON line per event.
//
// Pure observer: it emits nothing on stdout, always exits 0, and must
// never affect the tool call or the session it is watching. Registering
// it for all twelve event names the spec documents is itself part of the
// spike: an event that exists will land here with its real fields, and a
// name that never fires is a finding too.

"use strict";

process.on("uncaughtException", () => process.exit(0));
process.on("unhandledRejection", () => process.exit(0));

let raw = "";
process.stdin.setEncoding("utf8");
process.stdin.on("error", () => process.exit(0));
process.stdin.on("data", (c) => {
  raw += c;
});
process.stdin.on("end", () => {
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
    // Best-effort by design. Nothing here is worth surfacing.
  }
  process.exit(0);
});
