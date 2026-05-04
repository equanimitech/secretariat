import { useCallback, useEffect, useState } from 'react'
import { cn } from '@/lib/utils'
import { commands } from '@/lib/bindings'
import { ReviewSurface } from '@/components/secretariat/ReviewSurface'
import { Onboarding } from '@/components/secretariat/Onboarding'

interface MainWindowContentProps {
  children?: React.ReactNode
  className?: string
}

type Bootstrap =
  | { kind: 'loading' }
  | { kind: 'onboarding' }
  | { kind: 'ready' }

/// Routes between the onboarding wizard and the review surface based on
/// whether the principal has both a profile and an identity. Per the
/// pitch (`docs/pitches/2026-05-04-onboarding-wizard.md`), the wizard
/// runs once and never again — re-entry only happens if the user closes
/// the app mid-wizard, in which case `init_identity` is idempotent and
/// `set_profile` overwrites cleanly.
export function MainWindowContent({
  children,
  className,
}: MainWindowContentProps) {
  const [state, setState] = useState<Bootstrap>({ kind: 'loading' })

  const refresh = useCallback(async () => {
    const [profile, identity] = await Promise.all([
      commands.getProfile(),
      commands.currentIdentity(),
    ])
    const hasProfile =
      profile.status === 'ok' && profile.data !== null
    const hasIdentity =
      identity.status === 'ok' && identity.data !== null
    setState({
      kind: hasProfile && hasIdentity ? 'ready' : 'onboarding',
    })
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return (
    <div className={cn('flex h-full flex-col bg-background', className)}>
      {children ?? (
        <>
          {state.kind === 'loading' && null}
          {state.kind === 'onboarding' && <Onboarding onComplete={refresh} />}
          {state.kind === 'ready' && <ReviewSurface />}
        </>
      )}
    </div>
  )
}
