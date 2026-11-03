/** Fictional preview data, not Tracker DTOs or an API contract. */
export interface Section {
  id: string
  title: string
  description: string
  criteria: string
  requirements: string
  result: { summary: string; checks: string } | null
  resetReason?: string
}

export interface Plan {
  revision: number
  requirementsRevision: number
  overview: string
  author: string
  reason: string
  sections: Section[]
}

export const scenarios = [
  ['normal', 'План · частично выполнен'],
  ['empty', 'План отсутствует'],
  ['single', 'Один раздел'],
  ['long', 'Длинный план'],
  ['loading', 'Загрузка'],
  ['error', 'Ошибка чтения'],
  ['readonly', 'Только чтение'],
  ['busy', 'Агент работает'],
  ['conflict', 'Конфликт при сохранении'],
  ['stale', 'Требования изменились'],
  ['save-error', 'Ошибка записи'],
  ['revoked', 'Доступ отозван'],
] as const
export type Scenario = (typeof scenarios)[number][0]

export function scenarioFrom(value: string | null): Scenario {
  return scenarios.find(([key]) => key === value)?.[0] ?? 'normal'
}

export function fixture(scenario: Scenario): Plan | null {
  if (scenario === 'empty') return null
  const plan: Plan = {
    revision: 2,
    requirementsRevision: 3,
    author: 'Архитектор',
    reason: 'Уточнены проверки сохранения и доступа',
    overview:
      'Используем существующие профиль пользователя и форму настроек. Сохраняем настройки на сервере, проверяем доступ к своему профилю. Новые сервисы не нужны. Общий результат проверяем после повторного входа.',
    sections: [
      {
        id: 'storage',
        title: 'Хранение и API настроек',
        description:
          'Добавить настройки уведомлений в существующий профиль. Сохранение доступно только владельцу; повтор запроса безопасен.',
        criteria:
          'Настройки сохраняются и читаются повторно. Чужой профиль недоступен. Обновление совместимо с существующими данными.',
        requirements: 'К1 — сохранение настроек; К3 — доступ только владельцу',
        result: {
          summary: 'Реализовано сохранение профиля и проверка владельца.',
          checks: `Изменения: commit example-a1b2c3
Проверки: unit 12/12; PostgreSQL 5/5 (вымышленный пример)`,
        },
      },
      {
        id: 'form',
        title: 'Форма настроек уведомлений',
        description:
          'Дополнить существующую форму. Показать загрузку, сохранение и восстанавливаемую ошибку, сохранить ввод при отказе.',
        criteria:
          'Форма показывает сохранённые значения. Повтор после ошибки не теряет выбор. Все поля доступны с клавиатуры.',
        requirements: 'К1 — сохранение настроек; К2 — понятная ошибка',
        result: null,
      },
      {
        id: 'acceptance',
        title: 'Проверка целого сценария',
        description:
          'Проверить связь формы и API на общем кандидате. После повторного входа сохранённые значения должны восстановиться.',
        criteria:
          'Сценарий работает после нового входа. Проверены отказ доступа, повтор сохранения и мобильная ширина. Есть условия отката.',
        requirements: 'К1–К3 — итоговый сценарий и права',
        result: null,
      },
    ],
  }
  if (scenario === 'single')
    plan.sections = plan.sections.slice(0, 1).map((s) => ({ ...s, result: null }))
  if (scenario === 'long') {
    plan.overview +=
      ' Данные остаются в существующем профиле. Изменение контракта должно сохранять прежние значения и не затрагивать чужие настройки.'.repeat(
        16,
      )
    plan.sections = Array.from({ length: 8 }, (_, i) => ({
      ...plan.sections[i % 3]!,
      id: `section-${i}`,
      title: `${i + 1}. ${plan.sections[i % 3]!.title}`,
      description: plan.sections[i % 3]!.description.repeat(4),
    }))
  }
  return plan
}

export function revise(current: Plan | null, draft: Plan, reason: string): Plan {
  const sameInputs =
    current?.overview === draft.overview &&
    current?.requirementsRevision === draft.requirementsRevision
  const sections = draft.sections.map((section) => {
    const before = current?.sections.find((item) => item.id === section.id)
    const unchanged =
      sameInputs &&
      before &&
      before.description === section.description &&
      before.criteria === section.criteria &&
      before.requirements === section.requirements
    return { ...section, result: unchanged ? before.result : null }
  })
  return {
    ...draft,
    revision: (current?.revision ?? 0) + 1,
    author: 'Участник проекта',
    reason,
    sections,
  }
}

export function remoteRevision(plan: Plan): Plan {
  return {
    ...structuredClone(plan),
    revision: plan.revision + 1,
    author: 'Архитектор',
    reason: 'Параллельное уточнение архитектуры',
    overview: `${plan.overview} Новая правка архитектора: повтор сохранения не должен отправлять уведомление дважды.`,
  }
}
