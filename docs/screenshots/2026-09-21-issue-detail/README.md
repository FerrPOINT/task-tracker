# Issue detail QA, 2026-09-21

Isolated Chromium preview of the real `IssueDetailPage` with synthetic issue and API responses. It uses the same page width and padding as `AppShell`, but does not validate live SSO or server data.

Full-page screenshots: `375`, `1920`, and `2560` px in light and dark themes. `delete-error.png` records the failed deletion state.

Chromium QA: 7/7 scenarios passed after API fixtures were corrected to match array contracts. The page remained rendered after network idle, with no document overflow, page errors, or serious/critical axe violations under WCAG 2.0/2.1 A/AA. A failed DELETE kept confirmation open with a localized error; cancel released page interaction, and reopening showed no stale error. Live SSO and real API acceptance remain separate gates.
