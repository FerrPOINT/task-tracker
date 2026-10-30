# Общие действия: source UI evidence

[CommentItem на ширине 375 px](tracker-comment-dark-375.png) отрендерен из
настоящего frontend/src/features/comments/ui/CommentList.tsx с действующим
Base UI `c2ca61094a904706fceeda8eb8f3b3cfd2e7cdfc`.
Автор, комментарий и callbacks синтетические; приложение, Auth и API не
запускаются. Родительские запросы редактирования/удаления не исполняются.

IAB проверил CommentItem в 12 сочетаниях light/dark/gray и native iframe
320/375/1920/2560: перенос длинного имени, отсутствие overflow, targets
40/44 px и однократные локальные callbacks. Вместе с публичной формой Base
и RadioGroup проверены 36 сочетаний. В форме сохранены Enter/reset;
выбор значения не превращается в действие записи.

Проверены served source stamp и hashes исходников при закрытии собственного
QA host. Это source-компонент, не screenshot установленной страницы.
Снимок с обрезанным заголовком отклонён; здесь сохранён полный кадр.
Full product/authenticated gate и browser zoom 200% остаются отдельно.
