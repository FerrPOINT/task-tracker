# Project custom fields QA, 2026-09-21

Isolated Chromium preview of the real `ProjectCustomFieldsPage` with synthetic API responses. The preview uses page-like padding, but does not validate the full app shell, live SSO, or server data.

Full-page screenshots: `320`, `375`, `1920`, and `2560` px in light and dark themes. `375-en.png` records the English confirmation; `delete-error.png` records the failed deletion state.

Chromium QA: 11/11 scenarios passed. Comma-separated options remained editable and reached the POST request as three values. A failed DELETE kept confirmation open with a localized error and succeeded on retry. There was no document overflow, page error, or serious/critical axe violation under WCAG 2.0/2.1 A/AA. Main controls and delete targets were at least 44 px high at mobile widths. Live SSO and real API acceptance remain separate gates.
