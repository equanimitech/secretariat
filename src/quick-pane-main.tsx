import ReactDOM from 'react-dom/client'
import QuickPaneApp from './components/quick-pane/QuickPaneApp'
import './quick-pane.css'
import { installExternalLinkHandler } from './lib/external-links'

installExternalLinkHandler()

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <QuickPaneApp />
)
