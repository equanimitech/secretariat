// Sidebar toggles stay hidden for v0.3 (no sidebars rendered). The
// Settings button is surfaced again in v0.3 since the Settings panes
// (Profile, Paths, Shortcut, Relay, Integrations) are the principal's
// path into Secretariat configuration.
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { useUIStore as _useUIStore } from '@/store/ui-store'
import {
  executeCommand,
  useCommandContext,
} from '@/lib/commands'
import {
  PanelLeft as _PanelLeft,
  PanelLeftClose as _PanelLeftClose,
  PanelRight as _PanelRight,
  PanelRightClose as _PanelRightClose,
  Settings,
} from 'lucide-react'

/**
 * Left-side toolbar actions (sidebar toggle).
 * Hidden for v0.2.x — no sidebars rendered. Function returns null.
 */
export function TitleBarLeftActions() {

  // Left-sidebar-toggle button hidden for v0.2.x (no sidebars rendered).
  // Preserved for repurposing.
  //
  // return (
  //   <div className="flex items-center gap-1">
  //     <Button
  //       onClick={toggleLeftSidebar}
  //       variant="ghost"
  //       size="icon"
  //       className="h-6 w-6 text-foreground/70 hover:text-foreground"
  //       title={t(
  //         leftSidebarVisible
  //           ? 'titlebar.hideLeftSidebar'
  //           : 'titlebar.showLeftSidebar'
  //       )}
  //     >
  //       {leftSidebarVisible ? (
  //         <PanelLeftClose className="h-3 w-3" />
  //       ) : (
  //         <PanelLeft className="h-3 w-3" />
  //       )}
  //     </Button>
  //   </div>
  // )
  return null
}

/**
 * Right-side toolbar actions. Surfaces the Settings button — primary
 * entrypoint for the principal into Profile / Paths / Shortcut / Relay
 * / Integrations panes. Sidebar toggles stay hidden for v0.3.
 */
export function TitleBarRightActions() {
  const { t } = useTranslation()
  const commandContext = useCommandContext()

  const handleOpenPreferences = async () => {
    const result = await executeCommand('open-preferences', commandContext)
    if (!result.success && result.error) {
      commandContext.showToast(result.error, 'error')
    }
  }

  return (
    <div className="flex items-center gap-1">
      <Button
        onClick={handleOpenPreferences}
        variant="ghost"
        size="icon"
        className="h-6 w-6 text-foreground/70 hover:text-foreground"
        title={t('titlebar.settings')}
      >
        <Settings className="h-3 w-3" />
      </Button>
    </div>
  )
}

interface TitleBarTitleProps {
  title?: string
}

/**
 * Centered title for the title bar.
 * Uses absolute positioning to stay centered regardless of other content.
 */
export function TitleBarTitle({ title = 'Secretariat' }: TitleBarTitleProps) {
  return (
    <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2">
      <span className="text-sm font-medium text-foreground/80">{title}</span>
    </div>
  )
}

/**
 * Combined toolbar content for simple layouts.
 * Use this for Linux or when you want all toolbar items in one fragment.
 *
 * For more control, use TitleBarLeftActions, TitleBarRightActions, and TitleBarTitle separately.
 */
export function TitleBarContent({ title = 'Secretariat' }: TitleBarTitleProps) {
  return (
    <>
      <TitleBarLeftActions />
      <TitleBarTitle title={title} />
      <TitleBarRightActions />
    </>
  )
}
