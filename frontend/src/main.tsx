import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { I18nextProvider } from 'react-i18next'
import { Toaster } from 'sonner'
import i18n from './shared/i18n/config'
import { RouterProvider } from 'react-router'
import { router } from './app/router'
import { ThemeProvider } from './shared/lib/theme'
import './index.css'

const queryClient = new QueryClient()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <RouterProvider router={router} />
          <Toaster theme="dark" />
        </ThemeProvider>
      </QueryClientProvider>
    </I18nextProvider>
  </StrictMode>,
)
