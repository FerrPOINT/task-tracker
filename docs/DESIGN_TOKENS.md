# Design Tokens — Task Tracker

## 1. Overview

Design tokens — единый источник truth для цветов, отступов, типографики, скруглений, теней, z-index. Используются в CSS-переменных и Tailwind config.

Файлы:

- `docs/DESIGN_TOKENS.md` — документация.
- `frontend/src/styles/tokens.css` — CSS variables.

## 2. Color Palette

### 2.1 Base

| Token | Light | Dark |
|-------|-------|------|
| `--background` | `#ffffff` | `#09090b` |
| `--foreground` | `#18181b` | `#fafafa` |
| `--card` | `#ffffff` | `#18181b` |
| `--card-foreground` | `#18181b` | `#fafafa` |
| `--popover` | `#ffffff` | `#18181b` |
| `--popover-foreground` | `#18181b` | `#fafafa` |
| `--primary` | `#2563eb` | `#3b82f6` |
| `--primary-foreground` | `#ffffff` | `#ffffff` |
| `--secondary` | `#f4f4f5` | `#27272a` |
| `--secondary-foreground` | `#18181b` | `#fafafa` |
| `--muted` | `#f4f4f5` | `#27272a` |
| `--muted-foreground` | `#71717a` | `#a1a1aa` |
| `--accent` | `#f4f4f5` | `#27272a` |
| `--accent-foreground` | `#18181b` | `#fafafa` |
| `--destructive` | `#ef4444` | `#ef4444` |
| `--destructive-foreground` | `#ffffff` | `#ffffff` |
| `--border` | `#e4e4e7` | `#27272a` |
| `--input` | `#e4e4e7` | `#27272a` |
| `--ring` | `#2563eb` | `#3b82f6` |

### 2.2 Status Colors

| Status | Color |
|--------|-------|
| `--status-new` | `#71717a` |
| `--status-indeterminate` | `#3b82f6` |
| `--status-done` | `#22c55e` |
| `--status-danger` | `#ef4444` |
| `--status-warning` | `#f59e0b` |

### 2.3 Priority Colors

| Priority | Color |
|----------|-------|
| `--priority-highest` | `#dc2626` |
| `--priority-high` | `#f97316` |
| `--priority-medium` | `#eab308` |
| `--priority-low` | `#3b82f6` |
| `--priority-lowest` | `#71717a` |

### 2.4 Issue Type Colors

| Type | Color |
|------|-------|
| `--type-epic` | `#9333ea` |
| `--type-story` | `#16a34a` |
| `--type-task` | `#2563eb` |
| `--type-bug` | `#dc2626` |
| `--type-subtask` | `#71717a` |

## 3. Spacing Scale

| Token | Value |
|-------|-------|
| `--space-1` | `0.25rem` (4px) |
| `--space-2` | `0.5rem` (8px) |
| `--space-3` | `0.75rem` (12px) |
| `--space-4` | `1rem` (16px) |
| `--space-5` | `1.25rem` (20px) |
| `--space-6` | `1.5rem` (24px) |
| `--space-8` | `2rem` (32px) |
| `--space-10` | `2.5rem` (40px) |
| `--space-12` | `3rem` (48px) |
| `--space-16` | `4rem` (64px) |

## 4. Typography

### 4.1 Font Family

| Token | Value |
|-------|-------|
| `--font-sans` | `Inter, system-ui, sans-serif` |
| `--font-mono` | `JetBrains Mono, ui-monospace, monospace` |

### 4.2 Font Sizes

| Token | Value |
|-------|-------|
| `--text-xs` | `0.75rem` |
| `--text-sm` | `0.875rem` |
| `--text-base` | `1rem` |
| `--text-lg` | `1.125rem` |
| `--text-xl` | `1.25rem` |
| `--text-2xl` | `1.5rem` |
| `--text-3xl` | `1.875rem` |

### 4.3 Font Weights

| Token | Value |
|-------|-------|
| `--font-normal` | `400` |
| `--font-medium` | `500` |
| `--font-semibold` | `600` |
| `--font-bold` | `700` |

### 4.4 Line Heights

| Token | Value |
|-------|-------|
| `--leading-none` | `1` |
| `--leading-tight` | `1.25` |
| `--leading-normal` | `1.5` |
| `--leading-relaxed` | `1.625` |

## 5. Border Radius

| Token | Value |
|-------|-------|
| `--radius-sm` | `0.25rem` |
| `--radius-md` | `0.375rem` |
| `--radius-lg` | `0.5rem` |
| `--radius-xl` | `0.75rem` |
| `--radius-2xl` | `1rem` |
| `--radius-full` | `9999px` |

## 6. Shadows

| Token | Value |
|-------|-------|
| `--shadow-sm` | `0 1px 2px 0 rgb(0 0 0 / 0.05)` |
| `--shadow-md` | `0 4px 6px -1px rgb(0 0 0 / 0.1)` |
| `--shadow-lg` | `0 10px 15px -3px rgb(0 0 0 / 0.1)` |
| `--shadow-xl` | `0 20px 25px -5px rgb(0 0 0 / 0.1)` |

## 7. Z-Index

| Token | Value |
|-------|-------|
| `--z-base` | `0` |
| `--z-dropdown` | `100` |
| `--z-sticky` | `200` |
| `--z-popover` | `300` |
| `--z-modal` | `400` |
| `--z-toast` | `500` |
| `--z-tooltip` | `600` |

## 8. Layout

| Token | Value |
|-------|-------|
| `--topbar-height` | `3.5rem` |
| `--sidebar-width` | `16rem` |
| `--sidebar-collapsed-width` | `3.5rem` |
| `--content-max-width` | `1400px` |

## 9. Animation

| Token | Value |
|-------|-------|
| `--duration-fast` | `150ms` |
| `--duration-normal` | `250ms` |
| `--duration-slow` | `350ms` |
| `--ease-in-out` | `cubic-bezier(0.4, 0, 0.2, 1)` |

## 10. Tailwind Mapping

```js
// frontend/tailwind.config.ts or theme plugin
export const theme = {
  extend: {
    colors: {
      background: 'var(--background)',
      foreground: 'var(--foreground)',
      card: 'var(--card)',
      'card-foreground': 'var(--card-foreground)',
      popover: 'var(--popover)',
      'popover-foreground': 'var(--popover-foreground)',
      primary: 'var(--primary)',
      'primary-foreground': 'var(--primary-foreground)',
      secondary: 'var(--secondary)',
      'secondary-foreground': 'var(--secondary-foreground)',
      muted: 'var(--muted)',
      'muted-foreground': 'var(--muted-foreground)',
      accent: 'var(--accent)',
      'accent-foreground': 'var(--accent-foreground)',
      destructive: 'var(--destructive)',
      'destructive-foreground': 'var(--destructive-foreground)',
      border: 'var(--border)',
      input: 'var(--input)',
      ring: 'var(--ring)',
    },
    borderRadius: {
      sm: 'var(--radius-sm)',
      md: 'var(--radius-md)',
      lg: 'var(--radius-lg)',
      xl: 'var(--radius-xl)',
    },
    fontFamily: {
      sans: 'var(--font-sans)',
      mono: 'var(--font-mono)',
    },
    spacing: {
      1: 'var(--space-1)',
      2: 'var(--space-2)',
      3: 'var(--space-3)',
      4: 'var(--space-4)',
      5: 'var(--space-5)',
      6: 'var(--space-6)',
      8: 'var(--space-8)',
      10: 'var(--space-10)',
      12: 'var(--space-12)',
      16: 'var(--space-16)',
    },
    boxShadow: {
      sm: 'var(--shadow-sm)',
      md: 'var(--shadow-md)',
      lg: 'var(--shadow-lg)',
      xl: 'var(--shadow-xl)',
    },
    zIndex: {
      dropdown: 'var(--z-dropdown)',
      sticky: 'var(--z-sticky)',
      popover: 'var(--z-popover)',
      modal: 'var(--z-modal)',
      toast: 'var(--z-toast)',
      tooltip: 'var(--z-tooltip)',
    },
  },
}
```

## 11. Theme Switching

```css
:root {
  color-scheme: light dark;
}

:root, .light {
  /* light values */
}

.dark {
  /* dark values */
}
```

По умолчанию тема `dark`. Переключение через `document.documentElement.classList.add('dark')`.
## References

- `docs/REACT_STYLING.md`
- `docs/UI_UX.md`
- `docs/FRONTEND_ARCHITECTURE.md`
