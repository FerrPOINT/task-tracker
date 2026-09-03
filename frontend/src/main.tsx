import { StrictMode, useEffect, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { I18nextProvider } from 'react-i18next'
import { Toaster } from 'sonner'
import i18n from './shared/i18n/config'
import { RouterProvider } from 'react-router'
import { router } from './app/router'
import { ThemeProvider } from '@sdlc/ui/lib'
import './index.css'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Transient 429s (per-IP burst limit) must not surface as "empty"
      // data: retry with backoff so a short burst resolves instead of
      // rendering a permanently empty page.
      retry: (failureCount, error) => {
        if (failureCount >= 4) return false
        const status = (error as { status?: number })?.status
        return status === undefined || status === 429 || status >= 500
      },
      retryDelay: (attempt) => Math.min(1000 * 2 ** attempt, 10_000),
    },
    mutations: { retry: false },
  },
})

function Boot() {
  const [ready, setReady] = useState(i18n.isInitialized)
  useEffect(() => {
    if (i18n.isInitialized) {
      setReady(true)
      return
    }
    const handler = () => setReady(true)
    i18n.on('initialized', handler)
    return () => {
      i18n.off('initialized', handler)
    }
  }, [])
  if (!ready) return null
  return (
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <RouterProvider router={router} />
          <Toaster theme="dark" />
        </ThemeProvider>
      </QueryClientProvider>
    </I18nextProvider>
  )
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Boot />
  </StrictMode>,
)
