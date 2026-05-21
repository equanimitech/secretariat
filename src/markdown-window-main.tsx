import React from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClientProvider } from '@tanstack/react-query'
import { Toaster } from 'sonner'
import './markdown-window.css'
import { ThemeProvider } from './components/ThemeProvider'
import { MarkdownWindow } from './components/markdown/MarkdownWindow'
import { queryClient } from './lib/query-client'
import { installExternalLinkHandler } from './lib/external-links'

installExternalLinkHandler()

const params = new URLSearchParams(window.location.search)
const rawPath = params.get('path') ?? ''
const filePath = decodeURIComponent(rawPath)

const rootEl = document.getElementById('root')
if (!rootEl) throw new Error('root element missing')

ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <MarkdownWindow filePath={filePath} />
        <Toaster />
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>
)
