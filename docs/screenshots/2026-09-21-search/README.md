# Search results QA, 2026-09-21

Isolated Chromium preview of the real `SearchPage` with synthetic issue data. The preview matches the page width and padding of `AppShell`; it does not validate live SSO or server data.

Full-page screenshots: `320`, `375`, `1920`, and `2560` px in light and dark themes.

Chromium QA: 8/8 scenarios passed. The issue link has a nonzero focus surface of at least 40 px in height. No document overflow or serious/critical axe violations under WCAG 2.0/2.1 A/AA. Live SSO and real API acceptance remain separate gates.
