#!/usr/bin/env node
/**
 * Bridge: agy (Antigravity CLI) hooks -> GitNexus Gemini-CLI AfterTool hook.
 *
 * agy's PostToolUse may only answer {}, and only PreInvocation can inject
 * context (docs: builtin skill agy-customizations, hooks.md). So:
 *   PostToolUse   {toolCall, error}  -> run the GitNexus hook, park its
 *                                       additionalContext per conversation
 *   PreInvocation {invocationNum}    -> hand parked context to the model as
 *                                       {injectSteps: [{ephemeralMessage}]}
 *
 * Install: copy to ~/.gemini/config/hooks/gitnexus-agy/hook.cjs and register it
 * under both PostToolUse and PreInvocation in ~/.gemini/config/hooks.json.
 */
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const HOOK = path.join(os.homedir(), '.gemini/config/hooks/gitnexus/gitnexus-antigravity-hook.cjs');
const PARK = path.join(os.tmpdir(), 'gitnexus-agy');

function translate(call) {
  const a = call.args || {};
  switch (call.name) {
    case 'grep_search':
      return ['search_file_content', { pattern: a.Query ?? a.query ?? a.Pattern ?? a.pattern }];
    case 'find_by_name':
      return ['glob', { pattern: a.Pattern ?? a.pattern }];
    case 'run_command':
      return ['run_shell_command', { command: a.CommandLine ?? a.command }];
    default:
      return [null, null];
  }
}

function gitnexusContext(input) {
  const [toolName, toolInput] = translate(input.toolCall || {});
  if (!toolName || !fs.existsSync(HOOK)) return null;
  const cwd = (input.workspacePaths || [])[0] || process.cwd();
  const r = spawnSync(process.execPath, [HOOK], {
    input: JSON.stringify({
      hook_event_name: 'AfterTool',
      cwd,
      tool_name: toolName,
      tool_input: toolInput,
      tool_response: input.error ? { error: String(input.error) } : {},
    }),
    cwd,
    encoding: 'utf8',
    timeout: 25000,
  });
  try {
    return JSON.parse(r.stdout || '{}').hookSpecificOutput?.additionalContext || null;
  } catch {
    return null;
  }
}

let input = {};
try {
  input = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');
} catch {}

let out = {};
const conv = String(input.conversationId || 'none').replace(/[^\w-]/g, '');
const parked = path.join(PARK, conv);
try {
  if (input.toolCall) {
    const ctx = gitnexusContext(input);
    if (ctx) {
      fs.mkdirSync(PARK, { recursive: true });
      fs.appendFileSync(parked, ctx + '\n\n');
    }
  } else if ('invocationNum' in input && fs.existsSync(parked)) {
    const ctx = fs.readFileSync(parked, 'utf8').trim();
    fs.unlinkSync(parked);
    if (ctx) out = { injectSteps: [{ ephemeralMessage: ctx }] };
  }
} catch {}
process.stdout.write(JSON.stringify(out));
