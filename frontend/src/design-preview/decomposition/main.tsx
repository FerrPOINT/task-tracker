import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router'
import { createInstance } from 'i18next'
import { I18nextProvider } from 'react-i18next'
import { DecompositionPreview } from './preview'
import './preview.css'

// Local labels only: no HTTP backend, production auth, API client or host bridge.
const i18n = createInstance()
void i18n
  .init({
    lng: 'ru',
    resources: { ru: { translation: {} } },
    interpolation: { escapeValue: false },
  })
  .then(() => {
    const router = createBrowserRouter([{ path: '*', element: <DecompositionPreview /> }])
    createRoot(document.getElementById('root')!).render(
      <I18nextProvider i18n={i18n}>
        <RouterProvider router={router} />
      </I18nextProvider>,
    )
  })
