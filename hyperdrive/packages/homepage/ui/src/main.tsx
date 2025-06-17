import React from 'react'
import ReactDOM from 'react-dom/client'
import AndroidHomescreen from './pages/AndroidHomescreen.tsx'
import './index.css'
import 'uno.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AndroidHomescreen />
  </React.StrictMode>,
)
