import { commands, type ComposeResult, type SendResult } from '@/lib/tauri-bindings'

/** Draft a Slack message from a document. Does not send. */
export async function compose(
  docPath: string,
  instruction: string,
): Promise<ComposeResult> {
  const res = await commands.dispatchCompose('slack', docPath, instruction)
  if (res.status === 'error') throw new Error(res.error)
  return res.data
}

/** Send a confirmed body verbatim to a Slack channel. */
export async function send(channel: string, body: string): Promise<SendResult> {
  const res = await commands.dispatchSend('slack', channel, body)
  if (res.status === 'error') throw new Error(res.error)
  return res.data
}
