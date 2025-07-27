import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import "@radix-ui/themes/styles.css";
import './index.css'

import { Theme } from "@radix-ui/themes";
import { Toast } from "radix-ui";

import App from './App.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Theme>
      <Toast.Provider>
        <App />
      </Toast.Provider>
    </Theme>
  </StrictMode>,
)
