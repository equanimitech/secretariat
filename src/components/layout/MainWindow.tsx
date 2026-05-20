import {
  ResizablePanelGroup,
  ResizablePanel,
  ResizableHandle,
} from '@/components/ui/resizable'
import { LeftSideBar } from './LeftSideBar'
import { PreferencesDialog } from '@/components/preferences/PreferencesDialog'
import { TitleBar } from '@/components/titlebar/TitleBar'
import { MainWindowContent } from './MainWindowContent'
import { Toaster } from 'sonner'
import { useTheme } from '@/hooks/use-theme'
import { useMainWindowEventListeners } from '@/hooks/useMainWindowEventListeners'
import { useUIStore } from '@/store/ui-store'
import { cn } from '@/lib/utils'

const LAYOUT = {
  leftSidebar: { default: 22, min: 15, max: 40 },
  main: { min: 40 },
} as const

export function MainWindow() {
  const { theme } = useTheme()
  const leftSidebarVisible = useUIStore(state => state.leftSidebarVisible)

  useMainWindowEventListeners()

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden rounded-[var(--app-corner-radius)] bg-background">
      <TitleBar />

      <div className="flex w-full flex-1 overflow-hidden">
        <ResizablePanelGroup direction="horizontal">
          <ResizablePanel
            defaultSize={LAYOUT.leftSidebar.default}
            minSize={LAYOUT.leftSidebar.min}
            maxSize={LAYOUT.leftSidebar.max}
            className={cn(!leftSidebarVisible && 'hidden')}
          >
            <LeftSideBar />
          </ResizablePanel>

          <ResizableHandle className={cn(!leftSidebarVisible && 'hidden')} />

          <ResizablePanel
            defaultSize={100 - LAYOUT.leftSidebar.default}
            minSize={LAYOUT.main.min}
          >
            <MainWindowContent />
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>

      <PreferencesDialog />
      <Toaster
        position="bottom-right"
        theme={
          theme === 'dark' ? 'dark' : theme === 'light' ? 'light' : 'system'
        }
        className="toaster group"
        toastOptions={{
          classNames: {
            toast:
              'group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border-border group-[.toaster]:shadow-lg',
            description: 'group-[.toast]:text-muted-foreground',
            actionButton:
              'group-[.toast]:bg-primary group-[.toast]:text-primary-foreground',
            cancelButton:
              'group-[.toast]:bg-muted group-[.toast]:text-muted-foreground',
          },
        }}
      />
    </div>
  )
}
