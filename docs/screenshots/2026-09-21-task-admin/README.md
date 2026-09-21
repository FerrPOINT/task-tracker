# Administration UI QA

Isolated browser QA with mock system settings and audit events. Full-page
screenshots cover settings and audit at 320, 375, 1920, and 2560 px in light
and dark themes. `settings-error.png` and `save-error.png` show retry and
failure feedback.

Chromium: 9 passed, including retry, save failure/retry, and audit pagination;
no horizontal overflow, page errors, or serious/critical axe findings. Mobile
tab targets are at least 44 px high.
