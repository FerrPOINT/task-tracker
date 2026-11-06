-- Original guard reproduces the completion pre-read failure before any write.
SELECT qa_expect_scope_denial('true','false','{}','native_task_scope_invalid');
