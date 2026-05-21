// Simplified main-window surface.
//
// Vertical stack of full-width cards — one per reviewable vault
// (`_self` first, then every org alias in `~/.secretariat/orgs/`).
// Clicking a card dispatches `commands.reviewOrg`, which opens the
// principal's chosen terminal at the vault's substrate root and runs
// `claude --agent review` (the org-local review subagent, with a
// graceful fallback when no agent file exists yet).
//
// No counts, no blobs, no triage UI — that's the MCP's job per
// `memory/project_mcp_is_primary_interface.md`. This surface is just
// the front door.

import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import {
  commands,
  type AppPreferences,
  type ReviewableOrg,
} from '@/lib/bindings'

export function OrgPicker() {
  const [orgs, setOrgs] = useState<ReviewableOrg[] | null>(null)
  const [preferences, setPreferences] = useState<AppPreferences | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setError(null)
    const [orgsRes, prefsRes] = await Promise.all([
      commands.listReviewableOrgs(),
      commands.loadPreferences(),
    ])
    if (orgsRes.status === 'ok') setOrgs(orgsRes.data)
    else setError(orgsRes.error)
    if (prefsRes.status === 'ok') setPreferences(prefsRes.data)
  }, [])

  useEffect(() => {
    // One-shot Tauri IPC fetch on mount; no external store to subscribe to.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void refresh()
  }, [refresh])

  const review = useCallback(
    async (alias: string) => {
      setBusy(alias)
      try {
        const result = await commands.reviewOrg(
          alias,
          preferences?.assistant_terminal ?? null,
          preferences?.assistant_command ?? null
        )
        if (result.status === 'error') {
          toast.error(`Review failed: ${result.error}`)
        }
      } finally {
        setBusy(null)
      }
    },
    [preferences]
  )

  return (
    <div className="flex h-full w-full items-center justify-center px-6 py-10">
      <div className="flex w-full max-w-md flex-col gap-3">
        {error && (
          <div className="rounded-md border border-destructive/40 bg-destructive/10 px-4 py-2 text-sm text-destructive">
            {error}
          </div>
        )}

        {orgs === null && (
          <div className="text-center text-sm text-muted-foreground">
            Loading…
          </div>
        )}

        {orgs?.map(o => (
          <OrgRow
            key={o.alias}
            org={o}
            busy={busy === o.alias}
            onClick={() => review(o.alias)}
          />
        ))}

        {orgs && orgs.length === 1 && (
          <p className="mt-2 text-center text-xs text-muted-foreground">
            No orgs yet — `sec orgs create &lt;alias&gt;` to add one.
          </p>
        )}
      </div>
    </div>
  )
}

function OrgRow({
  org,
  busy,
  onClick,
}: {
  org: ReviewableOrg
  busy: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={busy}
      className="group flex w-full items-center justify-between rounded-xl border border-border bg-card px-5 py-4 text-left transition-colors hover:bg-accent focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 disabled:opacity-60"
    >
      <span className="flex flex-col">
        <span className="text-base font-medium text-foreground">
          Review {org.display_name}
        </span>
        <span className="mt-0.5 truncate text-xs text-muted-foreground">
          {org.root_path}
        </span>
      </span>
      <span className="text-sm text-muted-foreground transition-transform group-hover:translate-x-0.5">
        {busy ? '…' : '→'}
      </span>
    </button>
  )
}
