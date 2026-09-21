# Project trash QA, 2026-09-21

Isolated Chromium preview of the real `ProjectTrashPage` with synthetic API responses. The preview uses page-like padding, but does not validate the full app shell, live SSO, or server data.

Full-page screenshots: `320`, `375`, `1920`, and `2560` px in light and dark themes. `purge-error.png` records the failed permanent deletion state.

Chromium QA: 9/9 scenarios passed. There was no document overflow, page error, or serious/critical axe violation under WCAG 2.0/2.1 A/AA. At mobile widths, action targets were at least 44 px high. A failed purge kept confirmation open with a localized error; cancel restored page interaction, and reopening showed no stale error. A failed restore showed a localized toast. Live SSO and real API acceptance remain separate gates.
