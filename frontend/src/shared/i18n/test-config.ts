import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'

i18n.use(initReactI18next).init({
  lng: 'ru',
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
  resources: {
    ru: {
      translation: {
        common: {
          save: 'Сохранить',
          cancel: 'Отмена',
          edit: 'Изменить',
          delete: 'Удалить',
          loading: 'Загрузка...',
        },
        comments: {
          body: 'Комментарий',
          placeholder: 'Напишите комментарий...',
          add: 'Добавить комментарий',
          empty: 'Пока нет комментариев.',
          unknown: 'Неизвестный пользователь',
          deleteConfirm: 'Удалить комментарий?',
          validation: { required: 'Введите текст комментария' },
        },
      },
    },
    en: {
      translation: {
        common: {
          save: 'Save',
          cancel: 'Cancel',
          edit: 'Edit',
          delete: 'Delete',
          loading: 'Loading...',
        },
        comments: {
          body: 'Comment',
          placeholder: 'Write a comment...',
          add: 'Add comment',
          empty: 'No comments yet.',
          unknown: 'Unknown user',
          deleteConfirm: 'Delete this comment?',
          validation: { required: 'Comment text is required' },
        },
      },
    },
  },
})

export default i18n
