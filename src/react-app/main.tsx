import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import { App } from './App'
import './styles.css'

if (import.meta.env.DEV) {
  void import('react-grab')
  void import('react-scan')
}

const root = document.querySelector('#root')

if (root instanceof HTMLElement) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  )
}
