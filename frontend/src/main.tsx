import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { I18nextProvider, useTranslation } from 'react-i18next'
import { Toaster } from '@pdlc/ui/ui'
import i18n from './shared/i18n/config'
import { RouterProvider } from 'react-router'
import { router } from './app/router'
import { ThemeProvider, PlatformProvider, PlatformServicesProvider } from '@pdlc/ui/lib'
import './index.css'
import { SpaceSessionBoundary } from './shared/auth/space-session-boundary'
import { relaySsoCallback, ssoNamespace } from '@pdlc/ui/sso'
import {
  capturePlatformReturn,
  getPlatformProduct,
  setPlatformQueryClient,
  platformQueryDefaults,
} from '@pdlc/ui/platform'

const queryClient = new QueryClient({ defaultOptions: platformQueryDefaults })
setPlatformQueryClient(queryClient)
capturePlatformReturn(import.meta.env.VITE_PLATFORM_ORIGIN)

function Boot() {
  const { ready } = useTranslation(undefined, { i18n, useSuspense: false })
  if (!ready) return null
  return (
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <PlatformProvider configUrl={import.meta.env.VITE_PLATFORM_BRANDING_URL ?? null}>
            <PlatformServicesProvider
              catalogUrl={import.meta.env.VITE_PLATFORM_SERVICES_URL ?? null}
            >
              <SpaceSessionBoundary>
                <RouterProvider router={router} />
              </SpaceSessionBoundary>
            </PlatformServicesProvider>
          </PlatformProvider>
          <Toaster />
        </ThemeProvider>
      </QueryClientProvider>
    </I18nextProvider>
  )
}

if (!relaySsoCallback(ssoNamespace(import.meta.env.VITE_AUTH_NAMESPACE))) {
  const root = createRoot(document.getElementById('root')!)
  getPlatformProduct()?.onRetire(() => {
    root.unmount()
    router.dispose()
  })
  root.render(
    <StrictMode>
      <Boot />
    </StrictMode>,
  )
}
