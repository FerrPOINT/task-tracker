# Tracker PDLC2: только необходимые producers

Baseline dd90ed8f. Общий стандарт и selector принадлежат Base; продукт сохраняет
свои команды, Node22.20/pnpm10.28.1/Rust1.88, workers и независимые pins.

1. Подключить exact descriptor/selector, разделить server/официальный CLI/frontend.
   Неизвестный input расширяет продукт. Docs/tests не запускают production build.
2. Root, CI и raw container helper используют один Base plan. No-change не означает
   artifact ready; cold bootstrap задаётся явно. Reuse квалифицируется отдельно.
3. Полный прежний CI и http_only сохраняются manual release; real-Git classifier
   integration выполняется там, pure selection остаётся в автоматическом unit.
4. Frontend unit/types/compile выполняются один раз; postinstall output передаётся
   дальше. Сначала ограничить Tailwind roots, затем исключить test-only из producer.
5. Recipe contexts соответствуют COPY closure. Fresh Rust target и source download
   caches сохраняют корректность; unqualified compiled CI target не восстанавливать.
6. Scoped source checks и actual Root no-change/unit/PlanOnly. Native artifacts,
   Vitest/Vite, resource/old-timestamp/corruption/queued/live gates остаются OPEN.

Вариант локального второго path classifier отклонён: потреблять existing Base
selector. Старый classifier сохраняется только для manual/legacy compatibility.
Runtime/данные/SDK/UI/skills/operator pins, app sources и lockfiles не менять.
