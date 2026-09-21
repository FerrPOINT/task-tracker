# Project list QA, 2026-09-21

Isolated Chromium preview of the real `ProjectsPage` component with synthetic project data. The preview uses the same page width and padding as `AppShell`; it does not validate live SSO or server data.

Full-page screenshots: `320`, `375`, `1920`, and `2560` px in light and dark themes. `delete-error.png` records the failed deletion state.

Chromium QA: 9/9 scenarios passed. No document overflow or serious/critical axe violations under WCAG 2.0/2.1 A/AA. Mobile action buttons measured at least 40 by 40 px. The failed DELETE returned 503; the dialog stayed open and showed a localized error. Live SSO and real API acceptance remain separate gates.
