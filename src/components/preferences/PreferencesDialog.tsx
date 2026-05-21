import { useState } from 'react'
import {
  User,
  FolderOpen,
  Keyboard,
  Radio,
  Plug,
  Brain,
  Info,
} from 'lucide-react'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from '@/components/ui/sidebar'
import { useUIStore } from '@/store/ui-store'
import { ProfilePane } from './panes/ProfilePane'
import { PathsPane } from './panes/PathsPane'
import { ShortcutPane } from './panes/ShortcutPane'
import { RelayPane } from './panes/RelayPane'
import { IntegrationsPane } from './panes/IntegrationsPane'
import { CognitionPane } from './panes/CognitionPane'
import { AboutPane } from './panes/AboutPane'

// Template panes (Appearance / General / Advanced) remain on disk under
// `panes/` but are not surfaced — they're scaffold leftovers. The
// Secretariat-shaped panes shipping in v0.3 are: Profile, Paths,
// Shortcut, Relay, Integrations.

type PreferencePane =
  | 'profile'
  | 'paths'
  | 'shortcut'
  | 'relay'
  | 'integrations'
  | 'cognition'
  | 'about'

const navigationItems = [
  { id: 'profile' as const, label: 'Profile', icon: User },
  { id: 'paths' as const, label: 'Paths', icon: FolderOpen },
  { id: 'shortcut' as const, label: 'Shortcut', icon: Keyboard },
  { id: 'relay' as const, label: 'Relay', icon: Radio },
  { id: 'integrations' as const, label: 'Integrations', icon: Plug },
  { id: 'cognition' as const, label: 'Cognition', icon: Brain },
  { id: 'about' as const, label: 'About', icon: Info },
] as const

export function PreferencesDialog() {
  const [activePane, setActivePane] = useState<PreferencePane>('profile')
  const preferencesOpen = useUIStore(state => state.preferencesOpen)
  const setPreferencesOpen = useUIStore(state => state.setPreferencesOpen)

  const getPaneTitle = (pane: PreferencePane): string => {
    return navigationItems.find(item => item.id === pane)?.label ?? pane
  }

  return (
    <Dialog open={preferencesOpen} onOpenChange={setPreferencesOpen}>
      <DialogContent className="overflow-hidden p-0 md:max-h-[600px] md:max-w-[900px] lg:max-w-[1000px] font-sans rounded-xl">
        <DialogTitle className="sr-only">Settings</DialogTitle>
        <DialogDescription className="sr-only">
          Manage your Secretariat profile, identity, and storage.
        </DialogDescription>

        <SidebarProvider className="items-start">
          <Sidebar collapsible="none" className="hidden md:flex">
            <SidebarContent>
              <SidebarGroup>
                <SidebarGroupContent>
                  <SidebarMenu>
                    {navigationItems.map(item => (
                      <SidebarMenuItem key={item.id}>
                        <SidebarMenuButton
                          asChild
                          isActive={activePane === item.id}
                        >
                          <button
                            onClick={() => setActivePane(item.id)}
                            className="w-full"
                          >
                            <item.icon />
                            <span>{item.label}</span>
                          </button>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    ))}
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            </SidebarContent>
          </Sidebar>

          <main className="flex flex-1 flex-col overflow-hidden">
            <header className="flex h-16 shrink-0 items-center gap-2">
              <div className="flex items-center gap-2 px-4">
                <Breadcrumb>
                  <BreadcrumbList>
                    <BreadcrumbItem className="hidden md:block">
                      <BreadcrumbLink asChild>
                        <span>Settings</span>
                      </BreadcrumbLink>
                    </BreadcrumbItem>
                    <BreadcrumbSeparator className="hidden md:block" />
                    <BreadcrumbItem>
                      <BreadcrumbPage>
                        {getPaneTitle(activePane)}
                      </BreadcrumbPage>
                    </BreadcrumbItem>
                  </BreadcrumbList>
                </Breadcrumb>
              </div>
            </header>

            <div className="flex flex-1 flex-col gap-4 overflow-y-auto p-4 pt-0 max-h-[calc(600px-4rem)]">
              {activePane === 'profile' && <ProfilePane />}
              {activePane === 'paths' && <PathsPane />}
              {activePane === 'shortcut' && <ShortcutPane />}
              {activePane === 'relay' && <RelayPane />}
              {activePane === 'integrations' && <IntegrationsPane />}
              {activePane === 'cognition' && <CognitionPane />}
              {activePane === 'about' && <AboutPane />}
            </div>
          </main>
        </SidebarProvider>
      </DialogContent>
    </Dialog>
  )
}
