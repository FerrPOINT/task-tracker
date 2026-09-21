# AppShell navigation and notifications QA

Full-page screenshots from an isolated AppShell harness with mocked projects,
notifications, and account data. The content area is a placeholder; these images
verify the shared header, sidebar, mobile drawer, and notification menu only.

- `320`, `375`, `1920`, and `2560` pixel widths in light and dark themes.
- `menu-*` shows the mobile navigation drawer at 320 and 375 pixels.
- `notifications-320.png` shows the open notification menu at 320 pixels.

The browser run checked horizontal overflow, 44px mobile header controls,
current-route selection, menu focus/Escape behavior, English labels, and serious
or critical axe violations on both closed and open surfaces. All 10 QA scenarios
passed on 2026-09-21.
