import type { EngineInterface, Register } from 'claude-code'

// The calls whose permission prompt waits for us, until we answer it
const asked = new Set<string>()

// Stores the status in the pane's @status option, as the settings hooks do
const report = async ($: EngineInterface, status: 'running' | 'idle') => {
  const pane = await $.env.get('TMUX_PANE')
  if (pane === undefined) return
  const since =
    status === 'idle'
      ? ['set-option', '-p', '-q', '-o', '-t', pane, '@claude_idle_since', String(Math.floor(Date.now() / 1000))]
      : ['set-option', '-p', '-u', '-t', pane, '@claude_idle_since']
  await $.process.run(['tmux', ...since, ';', 'set-option', '-p', '-t', pane, '@status', status, ';', 'wait-for', '-S', 'state-changed'])
}

export const register: Register = on => {
  on('tool.check', async ($, e, next) => {
    const verdict = await next(e)
    if (verdict.decision === 'ask' && e.approver === 'person' && e.tool_use_id !== undefined) asked.add(e.tool_use_id)
    return verdict
  })

  // A call's row redraws when it starts running, which is when we approved it, or when it ends without running, which is when we refused it
  // Refusing with Esc ends the turn, and Claude waits for us; refusing with a reason lets Claude go on
  on('ui.render', { component: 'ToolUse' }, async ($, e, next) => {
    const { tool_use_id, isRunning, isInterrupted, output } = e.props
    if (asked.has(tool_use_id) && (isRunning || output !== undefined)) {
      asked.delete(tool_use_id)
      void report($, isRunning || !isInterrupted ? 'running' : 'idle')
    }
    return next(e)
  })
}
