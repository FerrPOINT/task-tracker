import {
  useMutation as useReactMutation,
  useQueryClient,
  type DefaultError,
  type QueryClient,
  type UseMutationOptions,
  type UseMutationResult,
  type MutateOptions,
} from '@tanstack/react-query'
import {
  isCurrentSpaceActor,
  useSpaceSession,
  type SpaceSession,
} from '@/shared/auth/space-session'

interface Invocation<T> {
  value: T
  actor: SpaceSession | null
  scoped: boolean
}
export class SpaceMutationScopeError extends Error {
  readonly code = 'space_mutation_context_changed'
  constructor() {
    super('Контекст операции изменился. Проверьте исходный запрос перед повтором.')
  }
}
function current<T>(request: Invocation<T>): boolean {
  return !request.scoped || Boolean(request.actor && isCurrentSpaceActor(request.actor))
}
function requireCurrent<T>(request: Invocation<T>): void {
  if (!current(request)) throw new SpaceMutationScopeError()
}
/** Keep the initiating scope in variables: pending observer options can change on rerender. */
export function useScopedMutation<
  TData = unknown,
  TError = DefaultError,
  TVariables = void,
  TContext = unknown,
>(
  options: UseMutationOptions<TData, TError, TVariables, TContext>,
  queryClient?: QueryClient,
): UseMutationResult<TData, TError, TVariables, TContext> {
  const actor = useSpaceSession((state) => state.session)
  const scoped = import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true'
  const client = useQueryClient(queryClient)
  const resolved = client.defaultMutationOptions(options)
  const result = useReactMutation<TData, TError, Invocation<TVariables>, TContext>(
    {
      ...resolved,
      // Retrying a retired/unknown request belongs to its original-key owner protocol.
      retry: scoped ? false : resolved.retry,
      mutationFn: async (request, context) => {
        requireCurrent(request)
        if (!resolved.mutationFn) throw new Error('No mutationFn found')
        try {
          return await resolved.mutationFn(request.value, context)
        } finally {
          requireCurrent(request)
        }
      },
      onMutate: resolved.onMutate
        ? async (request, context) => {
            requireCurrent(request)
            try {
              return await resolved.onMutate!(request.value, context)
            } finally {
              requireCurrent(request)
            }
          }
        : undefined,
      onSuccess: async (data, request, value, context) => {
        requireCurrent(request)
        await resolved.onSuccess?.(data, request.value, value, context)
        requireCurrent(request)
      },
      onError: (error, request, value, context) => {
        if (current(request)) return resolved.onError?.(error, request.value, value, context)
      },
      onSettled: async (data, error, request, value, context) => {
        if (error === null) requireCurrent(request)
        if (current(request)) await resolved.onSettled?.(data, error, request.value, value, context)
        if (error === null) requireCurrent(request)
      },
    },
    client,
  )
  const capture = (value: TVariables): Invocation<TVariables> => ({
    value,
    scoped,
    actor: actor ? { ...actor, identity: { ...actor.identity } } : null,
  })
  function callbacks(
    input: MutateOptions<TData, TError, TVariables, TContext> | undefined,
  ): MutateOptions<TData, TError, Invocation<TVariables>, TContext> | undefined {
    if (!input) return undefined
    return {
      onSuccess: (data, request, value, context) => {
        if (current(request)) input.onSuccess?.(data, request.value, value, context)
      },
      onError: (error, request, value, context) => {
        if (current(request)) input.onError?.(error, request.value, value, context)
      },
      onSettled: (data, error, request, value, context) => {
        if (current(request)) input.onSettled?.(data, error, request.value, value, context)
      },
    }
  }
  // Only the variable envelope/call signatures change; TanStack still owns status.
  return {
    ...result,
    variables: result.variables?.value,
    mutate: (value, input) => result.mutate(capture(value), callbacks(input)),
    mutateAsync: (value, input) => result.mutateAsync(capture(value), callbacks(input)),
  } as UseMutationResult<TData, TError, TVariables, TContext>
}
