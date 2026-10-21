import { useEffect, useRef, useState, type ComponentProps } from 'react'
import { Button, toast, usePlatformServices } from '@pdlc/ui/ui'
import { getPlatformProduct, platformNavigate } from '@pdlc/ui/platform'
import { LoaderCircle, Plus } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { getProject } from '@/api/project'

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

export function newPmChatUrl(base: string, projectId?: string): string {
  const url = new URL(base)
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password)
    throw new Error('Адрес Fleet не подтверждён')
  if (projectId && (!uuid.test(projectId) || projectId === '00000000-0000-0000-0000-000000000000'))
    throw new Error('Проект недоступен')
  url.pathname = `${url.pathname.replace(/\/+$/, '')}/chats/new`
  url.search = ''
  url.hash = ''
  if (projectId) url.searchParams.set('project', projectId)
  return url.href
}

type Props = Omit<ComponentProps<typeof Button>, 'asChild' | 'onClick'> & {
  projectKey?: string
  contextPending?: boolean
  contextError?: string
}

export function CreateTaskAction({
  projectKey,
  contextPending = false,
  contextError,
  children,
  disabled,
  ...props
}: Props) {
  const { t } = useTranslation()
  const { services, source } = usePlatformServices()
  const [opening, setOpening] = useState(false)
  const inFlight = useRef(false)
  const request = useRef<AbortController | null>(null)
  useEffect(() => () => request.current?.abort(), [projectKey, contextPending, contextError])

  async function open() {
    if (inFlight.current || contextPending) return
    if (contextError) {
      toast.error(contextError)
      return
    }
    // Use only Base's configured host destinations or validated runtime catalog.
    // Compiled display defaults never authorize cross-product navigation.
    const product = getPlatformProduct()
    const base = product
      ? product.services.find((service) => service.key === 'fleet-control')?.origin
      : source === 'runtime'
        ? services.find((service) => service.key === 'fleet-control')?.ui_url
        : undefined
    if (!base) {
      toast.error('Fleet недоступен: адрес чатов отсутствует в каталоге сервисов')
      return
    }
    inFlight.current = true
    setOpening(true)
    const controller = new AbortController()
    request.current = controller
    try {
      newPmChatUrl(base)
      let projectId: string | undefined
      if (projectKey) {
        const project = await getProject(projectKey, controller.signal)
        if (project.key !== projectKey || !project.id) throw new Error('Проект недоступен')
        projectId = project.id
      }
      if (controller.signal.aborted) return
      const target = newPmChatUrl(base, projectId)
      // Base returns void, not a route-acceptance ACK. Do not close the current
      // drawer optimistically; host rejection must leave this screen intact.
      platformNavigate(target)
    } catch {
      if (!controller.signal.aborted)
        toast.error('Не удалось открыть чат PM. Проверьте доступность проекта и Fleet.')
    } finally {
      inFlight.current = false
      setOpening(false)
    }
  }

  const pending = opening || contextPending
  return (
    <Button
      {...props}
      type="button"
      disabled={disabled || pending}
      aria-busy={pending}
      aria-label={props['aria-label'] ?? t('navigation.create')}
      title={pending ? 'Загружаем проект…' : contextError}
      onClick={() => void open()}
    >
      {pending ? (
        <LoaderCircle className="h-4 w-4 animate-spin" aria-hidden />
      ) : (
        <Plus className="h-4 w-4" aria-hidden />
      )}
      {children ?? t('navigation.create')}
    </Button>
  )
}
