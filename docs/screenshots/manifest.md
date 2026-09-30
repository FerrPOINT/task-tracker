# Screenshot Manifest

README embeds one desktop representative for each implemented page layout
mode. Responsive captures are retained here as QA evidence, not README media.

| File | Route | Layout | Viewport | PNG dimensions | Theme/data |
|---|---|---|---|---|---|
| [wide.png](1920x1080/wide.png) | `/projects` | `wide` | 1920x1080 | 1920x1080 | default theme, deterministic fixture |
| [reading.png](1920x1080/reading.png) | `/issues/create` | `reading` | 1920x1080 | 1920x1080 | default theme, deterministic fixture |
| [detail-with-aside.png](1920x1080/detail-with-aside.png) | `/issues/issue-1` | `detail-with-aside` | 1920x1080 | 1920x1080 | default theme, deterministic fixture |
| [wide.png](375x812/wide.png) | `/projects` | `wide` | 375x812 | 375x812 | responsive QA |
| [reading.png](375x812/reading.png) | `/issues/create` | `reading` | 375x812 | 375x815 | responsive QA |
| [detail-with-aside.png](375x812/detail-with-aside.png) | `/issues/issue-1` | `detail-with-aside` | 375x812 | 375x1577 | responsive QA |

All captures are full-page Playwright evidence. Route and interaction coverage
for the rest of the product is maintained in `frontend/e2e/`.
