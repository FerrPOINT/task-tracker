import i18n from 'i18next'
import { sdlcLocales } from '@sdlc/ui/i18n'
import { initReactI18next } from 'react-i18next'
import en from './locales/en.json'
import ru from './locales/ru.json'

i18n.use(initReactI18next).init({
  defaultNS: 'task-tracker',
  fallbackNS: 'base',
  resources: {
    en: { base: sdlcLocales.en, 'task-tracker': en },
    ru: { base: sdlcLocales.ru, 'task-tracker': ru },
  },
  lng: 'ru',
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
  react: { useSuspense: false },
})

export default i18n
