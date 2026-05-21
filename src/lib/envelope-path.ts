/**
 * Classify a file path against the envelope-archive substrate.
 *
 * Domain rule (mirrors `secretariat_core::application::inbox_actions`):
 *  - "Envelope" = file under a `<queue>/envelopes/...` subtree.
 *  - "Archived envelope" = file under a `<queue>/archived/` directory
 *    (where `<queue>` is the same dir that holds `envelopes/`).
 *
 * Archive / unarchive actions are only valid against these two classes.
 * Other markdown files (drafts, captures, contracts) return neither flag.
 */
export interface EnvelopeArchiveState {
  isEnvelope: boolean
  isArchived: boolean
}

export function classifyEnvelopePath(filePath: string): EnvelopeArchiveState {
  const segs = filePath.split('/').filter(Boolean)
  return {
    isEnvelope: segs.includes('envelopes'),
    isArchived: segs.includes('archived'),
  }
}
