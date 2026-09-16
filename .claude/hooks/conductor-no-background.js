#!/usr/bin/env node
// PreToolUse hook: in a conductor-run session (RLX_CONDUCTOR=1), deny a `Bash` or `PowerShell` call
// that carries `run_in_background`.
//
// Nothing re-invokes a `claude -p` session. A session that starts a command in the background and
// ends its turn exits, its background task is killed, and the work it was waiting on is lost — after
// the commits it already made have landed (ADR-0205). A long command runs in the foreground, and the
// session's own timeout is what bounds it.
//
// This is one of three layers, none sufficient alone: the prompts and the conductor-mode sections
// say it, this hook refuses it, and lib/outcome.mjs's `backgroundOutstanding` detects a session that
// got one started anyway. Outside the conductor the environment variable is unset and this hook
// passes everything through. Wired up in .claude/settings.json under hooks.PreToolUse with matcher
// "Bash|PowerShell".

const { readFileSync } = require("fs");

/** True when a tool call asks for backgrounding. The CLI accepts the boolean and the string form. */
function wantsBackground(input) {
  const v = input?.run_in_background;
  return v === true || v === "true";
}

function decide(input, env) {
  if (env.RLX_CONDUCTOR !== "1") return false;
  return wantsBackground(input.tool_input);
}

module.exports = { decide, wantsBackground };

if (require.main === module) {
  const input = JSON.parse(readFileSync(0, "utf8") || "{}");
  if (!decide(input, process.env)) {
    process.stdout.write("{}");
    process.exit(0);
  }
  const command = String(input.tool_input?.command ?? "").replace(/\s+/g, " ").trim();
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason:
          `Blocked "${command.slice(0, 80)}": a conductor-run session never starts a command in the ` +
          `background and never arms a Monitor. Nothing re-invokes a headless session, so a session ` +
          `that backgrounds a command and ends its turn loses that work and parks the plan after its ` +
          `commits have landed. Run it in the foreground; the session's own timeout bounds it (ADR-0205).`,
      },
    }),
  );
  process.exit(0);
}
