-- Ordinary comments remain ordinary content. Only insertion under the already
-- authorized, transaction-local native task scope gains this narrow machine port.
ALTER FUNCTION tracker_authorize_project_content(text,text,jsonb,jsonb)
 RENAME TO tracker_authorize_project_content_before_configured_comments;

CREATE FUNCTION tracker_authorize_project_content(table_name text,operation text,previous jsonb,desired jsonb)
RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE caller jsonb; scope jsonb; author uuid;
BEGIN
 IF table_name='comments' AND operation='INSERT'
    AND NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
  caller:=tracker_current_native_caller();
  scope:=NULLIF(current_setting('pdlc.tracker.native_task_scope',true),'')::jsonb;
  IF scope IS NULL OR scope->'identity' IS DISTINCT FROM caller->'identity'
     OR scope->'validity' IS DISTINCT FROM caller->'validity'
     OR scope->'writable' IS DISTINCT FROM 'true'::jsonb
     OR caller->'writable' IS DISTINCT FROM 'true'::jsonb
     OR desired->>'issue_id' IS DISTINCT FROM scope->>'task_id' THEN
   RAISE EXCEPTION 'native_comment_task_scope_required' USING ERRCODE='42501';
  END IF;
  SELECT id INTO author FROM users WHERE central_sub=caller->'identity'->>'subject' AND is_active FOR SHARE;
  IF author IS NULL OR desired->>'author_id' IS DISTINCT FROM author::text THEN
   RAISE EXCEPTION 'native_comment_author_mismatch' USING ERRCODE='42501';
  END IF;
  PERFORM tracker_bind_native_task(scope-'spaces');
  RETURN;
 END IF;
 -- Editing/deleting an accepted comment keeps the existing ordinary content
 -- rules; completion checks its original body and update timestamp under lock.
 PERFORM tracker_authorize_project_content_before_configured_comments(table_name,operation,previous,desired);
END $$;
