import { useEffect } from 'react'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { ask, message } from '@tauri-apps/plugin-dialog'
import { onOpenUrl } from '@tauri-apps/plugin-deep-link'
import { initializeCommandSystem } from './lib/commands'
import { buildAppMenu, setupMenuLanguageListener } from './lib/menu'
import { initializeLanguage } from './i18n/language-init'
import { logger } from './lib/logger'
import { cleanupOldFiles } from './lib/recovery'
import { commands } from './lib/tauri-bindings'
import './App.css'
import { MainWindow } from './components/layout/MainWindow'
import { ThemeProvider } from './components/ThemeProvider'
import { ErrorBoundary } from './components/ErrorBoundary'
import { useSquareCornersEffect } from './hooks/useSquareCornersEffect'
import { watchPendingOpens } from './lib/markdown/open'

function App() {
  useSquareCornersEffect()

  // Initialize command system and cleanup on app startup
  useEffect(() => {
    logger.info('🚀 Frontend application starting up')
    initializeCommandSystem()
    logger.debug('Command system initialized')

    // Initialize language based on saved preference or system locale
    const initLanguageAndMenu = async () => {
      try {
        // Load preferences to get saved language
        const result = await commands.loadPreferences()
        const savedLanguage =
          result.status === 'ok' ? result.data.language : null

        // Initialize language (will use system locale if no preference)
        await initializeLanguage(savedLanguage)

        // Build the application menu with the initialized language
        await buildAppMenu()
        logger.debug('Application menu built')
        setupMenuLanguageListener()
      } catch (error) {
        logger.warn('Failed to initialize language or menu', { error })
      }
    }

    initLanguageAndMenu()

    // Clean up old recovery files on startup
    cleanupOldFiles().catch(error => {
      logger.warn('Failed to cleanup old recovery files', { error })
    })

    // Example of logging with context
    logger.info('App environment', {
      isDev: import.meta.env.DEV,
      mode: import.meta.env.MODE,
    })

    // Auto-updater logic - check for updates 5 seconds after app loads.
    // Uses Tauri's native ask()/message() dialogs (OS-level) instead of
    // browser confirm()/alert(), because the main window is often hidden
    // when these fire (per the macOS hide-on-close behavior in lib.rs)
    // and browser-level dialogs in a hidden webview never surface to the
    // user.
    const checkForUpdates = async () => {
      try {
        const update = await check()
        if (update) {
          logger.info(`Update available: ${update.version}`)

          const shouldUpdate = await ask(
            `Version ${update.version} is available. Install now?`,
            {
              title: 'Secretariat update available',
              kind: 'info',
              okLabel: 'Install',
              cancelLabel: 'Later',
            }
          )

          if (shouldUpdate) {
            try {
              await update.downloadAndInstall(event => {
                switch (event.event) {
                  case 'Started':
                    logger.info(`Downloading ${event.data.contentLength} bytes`)
                    break
                  case 'Progress':
                    logger.info(`Downloaded: ${event.data.chunkLength} bytes`)
                    break
                  case 'Finished':
                    logger.info('Download complete, installing...')
                    break
                }
              })

              const shouldRestart = await ask(
                'Update installed. Restart Secretariat now to use the new version?',
                {
                  title: 'Restart to apply update',
                  kind: 'info',
                  okLabel: 'Restart',
                  cancelLabel: 'Later',
                }
              )

              if (shouldRestart) {
                await relaunch()
              }
            } catch (updateError) {
              logger.error(`Update installation failed: ${String(updateError)}`)
              await message(String(updateError), {
                title: 'Update failed',
                kind: 'error',
              })
            }
          }
        }
      } catch (checkError) {
        logger.error(`Update check failed: ${String(checkError)}`)
        // Silent fail for update checks - don't bother user with network issues
      }
    }

    // Check for updates 5 seconds after app loads
    const updateTimer = setTimeout(checkForUpdates, 5000)

    // Deep link listener — `secretariat://<host>/v0/invite/<token>` URLs
    // arrive here when the user clicks "Open in Secretariat" on the
    // relay's HTML landing page (or pastes a URL into a registered
    // handler). Fires the Tauri claim command, which auto-runs init for
    // first-time recipients.
    let deepLinkUnsub: (() => void) | undefined
    onOpenUrl(async urls => {
      for (const url of urls) {
        logger.info(`Deep link received: ${url}`)
        const result = await commands.claimInviteUrl(url)
        if (result.status === 'ok') {
          logger.info('Invite claimed', { ...result.data })
          alert(
            `Connected to ${result.data.inviter_did}.\nYou can now exchange envelopes.`
          )
        } else {
          logger.error(`Claim failed: ${result.error}`)
          alert(`Could not claim invite:\n${result.error}`)
        }
      }
    })
      .then(unsub => {
        deepLinkUnsub = unsub
      })
      .catch(err => {
        logger.warn('Failed to register deep link handler', { error: err })
      })

    // Markdown reader/editor: drain PendingOpens and open windows for files
    // that arrived via RunEvent::Opened / single-instance argv.
    const unwatchPendingOpens = watchPendingOpens()

    return () => {
      clearTimeout(updateTimer)
      deepLinkUnsub?.()
      unwatchPendingOpens()
    }
  }, [])

  return (
    <ErrorBoundary>
      <ThemeProvider>
        <MainWindow />
      </ThemeProvider>
    </ErrorBoundary>
  )
}

export default App
