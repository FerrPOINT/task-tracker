# Authentication entry QA

Full-page screenshots from the real Task Tracker login and SSO callback routes.
The login route used `?logged_out=1` to keep Central Auth from starting during
visual QA. The callback image shows the recoverable error for missing callback
parameters. No real identity-provider session or profile was used.

- `login-*`: 320, 375, 1920, and 2560 pixel widths in light and dark themes.
- `callback-error-375.png`: invalid callback with a localized retry action.

Chromium passed 9 scenarios on 2026-09-21: no horizontal overflow or page
errors, no serious/critical axe violations, visible security notice, and
mobile buttons at least 44px in both dimensions.
