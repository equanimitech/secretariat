import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { usePlatform, type AppPlatform } from '@/hooks/use-platform'
import { useUIStore } from '@/store/ui-store'
import { MacOSWindowControls } from './MacOSWindowControls'
import { WindowsWindowControls } from './WindowsWindowControls'
import {
  TitleBarLeftActions,
  TitleBarRightActions,
  TitleBarTitle,
} from './TitleBarContent'
import { LinuxTitleBar } from './LinuxTitleBar'

interface TitleBarProps {
  className?: string
  title?: string
  /**
   * Force a specific platform for development/testing.
   * Only works in development builds.
   */
  forcePlatform?: AppPlatform
}

/**
 * Cross-platform title bar component.
 *
 * Renders platform-specific title bars:
 * - **macOS**: Custom title bar with traffic lights on LEFT
 * - **Windows**: Custom title bar with controls on RIGHT
 * - **Linux**: Toolbar only (native decorations provide window controls)
 *
 * Use `forcePlatform` prop in development to test other platform layouts.
 */
export function TitleBar({ className, title, forcePlatform }: TitleBarProps) {
  const { t } = useTranslation()
  const displayTitle = title ?? t('titlebar.default')
  const detectedPlatform = usePlatform()
  const isFullscreen = useUIStore(state => state.isFullscreen)

  // In development, allow forcing a platform for testing
  const platform =
    import.meta.env.DEV && forcePlatform ? forcePlatform : detectedPlatform

  // Linux uses native decorations, so render just the toolbar
  if (platform === 'linux') {
    return <LinuxTitleBar className={className} title={displayTitle} />
  }

  // Windows: controls on the right
  if (platform === 'windows') {
    return (
      <div
        data-tauri-drag-region
        className={cn(
          'relative flex h-8 w-full shrink-0 items-center justify-between border-b bg-background',
          className
        )}
      >
        {/* Left side - Actions */}
        <div className="flex items-center pl-2">
          <TitleBarLeftActions />
        </div>

        {/* Center - Title */}
        <TitleBarTitle title={displayTitle} />

        {/* Right side - Actions + Window Controls */}
        <div className="flex items-center">
          <TitleBarRightActions />
          <WindowsWindowControls />
        </div>
      </div>
    )
  }

  // macOS (default): traffic lights on the left.
  //
  // In native fullscreen we hide the custom traffic lights — macOS
  // already provides the menu-bar reveal at the top edge with native
  // controls, and rendering our own buttons there both overlaps that
  // reveal and confuses fullscreen exit (the green button no longer
  // means "fullscreen" once you're already in it). The titlebar row
  // itself stays so the left/right action menus remain reachable.
  return (
    <div
      data-tauri-drag-region
      className={cn(
        'relative flex h-8 w-full shrink-0 items-center justify-between border-b bg-background',
        className
      )}
    >
      {/* Left side - Window Controls + Actions */}
      <div className="flex items-center">
        {!isFullscreen && <MacOSWindowControls />}
        <TitleBarLeftActions />
      </div>

      {/* Center - Title */}
      <TitleBarTitle title={displayTitle} />

      {/* Right side - Actions */}
      <div className="flex items-center pr-2">
        <TitleBarRightActions />
      </div>
    </div>
  )
}
