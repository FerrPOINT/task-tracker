# React Styling Guide — Task Tracker

## 1. Цель

Шаблонизировать стили так, чтобы каждый отступ, цвет, размер и поведение компонентов не правились вручную, а брались из design tokens и переиспользуемых компонентов.

## 2. Основной подход

### 2.1 Tailwind CSS v4 + CSS Variables

- Все базовые токены вынесены в `frontend/src/styles/tokens.css`.
- Tailwind v4 использует `@theme` для маппинга CSS-переменных на utility-классы.
- В компонентах используем только Tailwind utility-классы; никаких inline styles и произвольных `style={{}}`.

### 2.2 shadcn/ui как база

- shadcn/ui предоставляет headless-компоненты на Radix, стилизованные через Tailwind.
- Все компоненты лежат в `frontend/src/components/ui/`.
- При необходимости кастомизации — расширяем через variants (CVA), не правим className в каждом месте.

### 2.3 Component Variants API (CVA)

Для компонентов с несколькими вариантами используем `class-variance-authority`.

```tsx
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-primary text-primary-foreground",
        secondary: "border-transparent bg-secondary text-secondary-foreground",
        destructive: "border-transparent bg-destructive text-destructive-foreground",
        outline: "text-foreground",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

export interface BadgeProps extends VariantProps<typeof badgeVariants> {
  children: React.ReactNode
  className?: string
}

export function Badge({ variant, className, children }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)}>{children}</span>
}
```

## 3. Управление отступами

### 3.1 Специализированные layout-компоненты

Вместо того чтобы писать `className="flex gap-4 p-4"` в каждом месте, создаём компоненты:

```tsx
// components/layout/Stack.tsx
import { cn } from "@/lib/utils"

interface StackProps {
  children: React.ReactNode
  gap?: 1 | 2 | 3 | 4 | 5 | 6 | 8
  direction?: "row" | "col"
  className?: string
}

export function Stack({ children, gap = 4, direction = "col", className }: StackProps) {
  return (
    <div className={cn(`flex gap-${gap}`, direction === "col" ? "flex-col" : "flex-row", className)}>
      {children}
    </div>
  )
}
```

### 3.2 Page-level spacing

```tsx
// layouts/AppLayout.tsx
export function AppLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-screen bg-background text-foreground">
      <Sidebar />
      <main className="flex-1 p-6">{children}</main>
    </div>
  )
}
```

### 3.3 Spacer component

```tsx
export function Spacer({ size = 4 }: { size?: 1 | 2 | 3 | 4 | 6 | 8 }) {
  return <div className={`h-${size}`} />
}
```

## 4. Цвета и темы

### 4.1 Использование токенов

```tsx
// правильно
<div className="bg-card text-card-foreground border-border">

// неправильно
<div className="bg-white text-black border-gray-200">
```

### 4.2 Semantic цвета для статусов

```tsx
function StatusBadge({ status }: { status: "new" | "done" | "indeterminate" | "danger" }) {
  const map = {
    new: "bg-status-new/10 text-status-new border-status-new/20",
    done: "bg-status-done/10 text-status-done border-status-done/20",
    indeterminate: "bg-status-indeterminate/10 text-status-indeterminate",
    danger: "bg-status-danger/10 text-status-danger",
  }
  return <Badge className={map[status]}>{status}</Badge>
}
```

## 5. Типографика

```tsx
function PageTitle({ children }: { children: React.ReactNode }) {
  return <h1 className="text-2xl font-semibold tracking-tight">{children}</h1>
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h2 className="text-lg font-medium">{children}</h2>
}

function Text({ children, muted }: { children: React.ReactNode; muted?: boolean }) {
  return <p className={cn("text-base", muted && "text-muted-foreground")}>{children}</p>
}
```

## 6. Карточки и поверхности

```tsx
function Card({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn("rounded-lg border bg-card p-4 shadow-sm", className)}>
      {children}
    </div>
  )
}
```

## 7. Формы

```tsx
import { Form, FormField, FormItem, FormLabel, FormControl, FormMessage } from "@/components/ui/form"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"

function IssueForm() {
  return (
    <Form {...form}>
      <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
        <FormField name="summary" render={({ field }) => (
          <FormItem>
            <FormLabel>Summary</FormLabel>
            <FormControl>
              <Input placeholder="Enter summary" {...field} />
            </FormControl>
            <FormMessage />
          </FormItem>
        )} />
        <Button type="submit">Create</Button>
      </form>
    </Form>
  )
}
```

## 8. Таблицы

```tsx
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow
} from "@/components/ui/table"
```

## 9. Состояния загрузки и ошибок

```tsx
function Loading({ className }: { className?: string }) {
  return <Skeleton className={cn("h-4 w-full", className)} />
}

function EmptyState({ title, description }: { title: string; description?: string }) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border border-dashed p-8 text-center">
      <p className="text-lg font-medium">{title}</p>
      {description && <p className="text-muted-foreground">{description}</p>}
    </div>
  )
}
```

## 10. Responsive

- Mobile-first.
- Используем `sm:`, `md:`, `lg:`, `xl:` префиксы.
- Layout breakpoints вынесены в tokens, но в классах пишем Tailwind-брейкпоинты.

```tsx
<div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
```

## 11. Не рекомендуется

| Плохо | Почему |
|-------|--------|
| Inline `style={{ marginTop: 16 }}` | Нет единой системы, сложно поддерживать |
| Произвольные px/rem в className | Разные отступы по всему приложению |
| Прямое указание hex-цветов | Не меняется в темной теме |
| Копирование одинаковых className | Дублирование, ошибки |
| `!important` | Конфликты, сложность |

## 12. Рекомендуется

| Хорошо | Почему |
|--------|--------|
| Переиспользуемые layout-компоненты | Stack, Card, PageTitle, Spacer |
| CVA для вариантов | Один компонент, множество состояний |
| CSS-переменные + Tailwind @theme | Автоматическая тёмная тема |
| shadcn/ui базовые компоненты | Консистентность, accessibility |
| `cn()` из `@/lib/utils` | Умное слияние классов |

## 13. Пример страницы

```tsx
import { PageTitle } from "@/components/typography/PageTitle"
import { Stack } from "@/components/layout/Stack"
import { Card } from "@/components/surfaces/Card"
import { Badge } from "@/components/ui/badge"

export function ProjectPage({ project }: { project: Project }) {
  return (
    <Stack gap={6}>
      <PageTitle>{project.name}</PageTitle>
      <Card>
        <Stack direction="row" gap={2}>
          <Badge variant="secondary">{project.key}</Badge>
          <Badge>{project.type}</Badge>
        </Stack>
      </Card>
    </Stack>
  )
}
```

## 14. Импорт порядка

```tsx
// 1. React / external libraries
import { useQuery } from "@tanstack/react-query"

// 2. Internal components
import { Card } from "@/components/surfaces/Card"
import { Stack } from "@/components/layout/Stack"

// 3. UI primitives
import { Button } from "@/components/ui/button"

// 4. Hooks / utils
import { useProject } from "@/hooks/useProject"
import { cn } from "@/lib/utils"

// 5. Types
import type { Project } from "@/types/project"
```

## 15. Файловая структура стилей

```
frontend/src/
├── styles/
│   ├── tokens.css          # CSS variables + @theme
│   ├── globals.css         # base styles
│   └── animations.css      # keyframes
├── components/
│   ├── ui/                 # shadcn/ui primitives
│   ├── layout/             # Stack, Grid, Spacer, AppLayout
│   ├── surfaces/           # Card, Panel, PageHeader
│   ├── typography/         # PageTitle, SectionTitle, Text
│   └── feedback/           # Loading, EmptyState, ErrorState
└── lib/
    └── utils.ts            # cn() helper
```
## References

- `docs/DESIGN_TOKENS.md`
- `docs/FRONTEND_ARCHITECTURE.md`
- `docs/UI_LIBRARIES.md`
