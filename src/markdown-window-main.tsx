import React from 'react'
import ReactDOM from 'react-dom/client'
import { Toaster } from 'sonner'
import './markdown-window.css'
import { ThemeProvider } from './components/ThemeProvider'
import { MarkdownWindow } from './components/markdown/MarkdownWindow'

const params = new URLSearchParams(window.location.search)
const rawPath = params.get('path') ?? ''
const filePath = decodeURIComponent(rawPath)

const rootEl = document.getElementById('root')
if (!rootEl) throw new Error('root element missing')

ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <ThemeProvider>
      <MarkdownWindow filePath={filePath} />
      <Toaster />
    </ThemeProvider>
  </React.StrictMode>,
)
