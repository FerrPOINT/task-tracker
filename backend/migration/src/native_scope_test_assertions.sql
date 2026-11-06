DO $$
DECLARE observed jsonb; row_data jsonb:='{"task_id":"00000000-0000-4000-8000-000000000003","project_id":"00000000-0000-4000-8000-000000000002"}';
BEGIN
 -- A retained read inside POST keeps the original identity and deadline,
 -- but the resulting scope cannot authorize the following business write.
 observed:=qa_bind_scope('true','false');
 IF observed->'writable'<>'false'::jsonb OR observed->'identity'<>(current_setting('pdlc.tracker.native_caller')::jsonb)->'identity'
    OR observed->'validity'<>(current_setting('pdlc.tracker.native_caller')::jsonb)->'validity' THEN
  RAISE EXCEPTION 'attenuation changed identity, clock or write authority';
 END IF;
 BEGIN
  PERFORM tracker_authorize_native_sdlc('sdlc_tasks','UPDATE',row_data,row_data);
  RAISE EXCEPTION 'retained scope authorized a write';
 EXCEPTION WHEN insufficient_privilege THEN
  IF SQLERRM<>'native_task_scope_required' THEN RAISE;END IF;
 END;
 PERFORM set_config('pdlc.tracker.native_issue_projection',jsonb_build_object(
  'task_id',row_data->>'task_id','status_id','00000000-0000-4000-8000-000000000007')::text,true);
 BEGIN
  PERFORM tracker_authorize_native_issue_projection('issues','UPDATE',
   jsonb_build_object('id',row_data->>'task_id','status_id','00000000-0000-4000-8000-000000000006','sprint_id',NULL),
   jsonb_build_object('id',row_data->>'task_id','status_id','00000000-0000-4000-8000-000000000007','sprint_id',NULL));
  RAISE EXCEPTION 'retained scope authorized an issue projection';
 EXCEPTION WHEN insufficient_privilege THEN
  IF SQLERRM<>'native_projection_required' THEN RAISE;END IF;
 END;
 BEGIN
  PERFORM tracker_authorize_native_task_content('comments','INSERT',jsonb_build_object('issue_id',row_data->>'task_id'));
  RAISE EXCEPTION 'retained scope authorized task content';
 EXCEPTION WHEN insufficient_privilege THEN
  IF SQLERRM<>'native_content_task_scope_required' THEN RAISE;END IF;
 END;
 -- Legitimate current writes and ordinary GET reads remain available.
 PERFORM qa_bind_scope('true','true');
 PERFORM tracker_authorize_native_sdlc('sdlc_tasks','UPDATE',row_data,row_data);
 PERFORM tracker_authorize_native_issue_projection('issues','UPDATE',
  jsonb_build_object('id',row_data->>'task_id','status_id','00000000-0000-4000-8000-000000000006','sprint_id',NULL),
  jsonb_build_object('id',row_data->>'task_id','status_id','00000000-0000-4000-8000-000000000007','sprint_id',NULL));
 PERFORM qa_bind_scope('false','false');
 PERFORM qa_expect_scope_denial('false','true','{}','native_task_scope_invalid');
 PERFORM qa_expect_scope_denial('true','"false"','{}','native_task_scope_invalid');
 PERFORM qa_expect_scope_denial('"true"','false','{}','native_task_scope_invalid');
 PERFORM qa_expect_scope_denial('true','false','{"identity":{"issuer":"https://native-scope.test","subject":"foreign"}}','native_task_scope_invalid');
 PERFORM qa_expect_scope_denial('true','false','{"validity":{"observed_at":1,"valid_until":2}}','native_task_scope_invalid');
 PERFORM qa_expect_scope_denial('true','false','{}','native_request_authority_expired',0);
 PERFORM qa_expect_scope_denial('true','false','{"project_id":"00000000-0000-4000-8000-000000000008"}','native_task_owner_changed');
 PERFORM qa_expect_scope_denial('true','false','{"task_id":"00000000-0000-4000-8000-000000000008"}','native_task_owner_changed');
 UPDATE projects SET archived_at=clock_timestamp();
 PERFORM qa_bind_scope('true','false');
 PERFORM qa_expect_scope_denial('true','true','{}','native_task_owner_changed');
 UPDATE projects SET archived_at=NULL;
 DELETE FROM project_members;
 PERFORM qa_expect_scope_denial('true','false','{}','native_project_role_required');
END $$;
