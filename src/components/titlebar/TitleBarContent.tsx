// Imports kept under underscore-aliases for v0.2.x — chrome restored by
// removing the underscore prefix and uncommenting the JSX in each
// function body. See `memory/project_settings_pane_shape.md`.
import { useTranslation as _useTranslation } from 'react-i18next'
import { Button as _Button } from '@/components/ui/button'
import { useUIStore as _useUIStore } from '@/store/ui-store'
import {
  executeCommand as _executeCommand,
  useCommandContext as _useCommandContext,
} from '@/lib/commands'
import {
  PanelLeft as _PanelLeft,
  PanelLeftClose as _PanelLeftClose,
  PanelRight as _PanelRight,
  PanelRightClose as _PanelRightClose,
  Settings as _Settings,
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
 * Right-side toolbar actions (settings, sidebar toggle).
 * Hidden for v0.2.x — function returns null.
 */
export function TitleBarRightActions() {
  // Settings + right-sidebar-toggle buttons hidden for v0.2.x
  // (memory/project_settings_pane_shape.md). Preserved for repurposing.
  //
  // const { t } = useTranslation()
  // const rightSidebarVisible = useUIStore(state => state.rightSidebarVisible)
  // const toggleRightSidebar = useUIStore(state => state.toggleRightSidebar)
  // const commandContext = useCommandContext()
  //
  // const handleOpenPreferences = async () => {
  //   const result = await executeCommand('open-preferences', commandContext)
  //   if (!result.success && result.error) {
  //     commandContext.showToast(result.error, 'error')
  //   }
  // }
  //
  // return (
  //   <div className="flex items-center gap-1">
  //     <Button
  //       onClick={handleOpenPreferences}
  //       variant="ghost"
  //       size="icon"
  //       className="h-6 w-6 text-foreground/70 hover:text-foreground"
  //       title={t('titlebar.settings')}
  //     >
  //       <Settings className="h-3 w-3" />
  //     </Button>
  //
  //     <Button
  //       onClick={toggleRightSidebar}
  //       variant="ghost"
  //       size="icon"
  //       className="h-6 w-6 text-foreground/70 hover:text-foreground"
  //       title={t(
  //         rightSidebarVisible
  //           ? 'titlebar.hideRightSidebar'
  //           : 'titlebar.showRightSidebar'
  //       )}
  //     >
  //       {rightSidebarVisible ? (
  //         <PanelRightClose className="h-3 w-3" />
  //       ) : (
  //         <PanelRight className="h-3 w-3" />
  //       )}
  //     </Button>
  //   </div>
  // )
  return null
}

interface TitleBarTitleProps {
  title?: string
}

/**
 * Centered title for the title bar.
 * Uses absolute positioning to stay centered regardless of other content.
 */
export function TitleBarTitle({ title = 'Tauri App' }: TitleBarTitleProps) {
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
export function TitleBarContent({ title = 'Tauri App' }: TitleBarTitleProps) {
  return (
    <>
      <TitleBarLeftActions />
      <TitleBarTitle title={title} />
      <TitleBarRightActions />
    </>
  )
}
