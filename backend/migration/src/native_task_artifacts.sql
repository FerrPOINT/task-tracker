-- A task capability can publish only the exact sealed row prepared by the
-- artifact repository. It cannot lend authority to ordinary attachments.
CREATE FUNCTION tracker_authorize_native_task_content(table_name text,operation text,desired jsonb)
RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE caller jsonb; scope jsonb; author uuid; artifact jsonb;
BEGIN
 caller:=tracker_current_native_caller();
 scope:=NULLIF(current_setting('pdlc.tracker.native_task_scope',true),'')::jsonb;
 IF operation<>'INSERT' OR table_name NOT IN ('comments','attachments')
    OR scope IS NULL OR scope->'identity' IS DISTINCT FROM caller->'identity'
    OR scope->'validity' IS DISTINCT FROM caller->'validity'
    OR scope->'writable' IS DISTINCT FROM 'true'::jsonb
    OR caller->'writable' IS DISTINCT FROM 'true'::jsonb
    OR desired->>'issue_id' IS DISTINCT FROM scope->>'task_id' THEN
  RAISE EXCEPTION 'native_content_task_scope_required' USING ERRCODE='42501';
 END IF;
 SELECT id INTO author FROM users WHERE central_sub=caller->'identity'->>'subject' AND is_active FOR SHARE;
 IF author IS NULL OR desired->>'author_id' IS DISTINCT FROM author::text THEN
  RAISE EXCEPTION 'native_content_author_mismatch' USING ERRCODE='42501';
 END IF;
 IF table_name='attachments' THEN
  artifact:=NULLIF(current_setting('pdlc.tracker.native_artifact',true),'')::jsonb;
  IF artifact IS NULL OR artifact IS DISTINCT FROM desired-'created_at'
     OR desired->>'storage_key' NOT LIKE 'sdlc-artifact-v1\_%' ESCAPE '\' THEN
   RAISE EXCEPTION 'native_sealed_artifact_required' USING ERRCODE='42501';
  END IF;
 END IF;
 PERFORM tracker_bind_native_task(scope-'spaces');
END $$;

CREATE OR REPLACE FUNCTION tracker_authorize_project_content(table_name text,operation text,previous jsonb,desired jsonb)
RETURNS void LANGUAGE plpgsql VOLATILE AS $$
BEGIN
 IF table_name IN ('comments','attachments') AND operation='INSERT'
    AND NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
  PERFORM tracker_authorize_native_task_content(table_name,operation,desired);
  RETURN;
 END IF;
 PERFORM tracker_authorize_project_content_before_configured_comments(table_name,operation,previous,desired);
END $$;

-- Recheck the same narrow authority after every blocking lock and through COMMIT.
CREATE OR REPLACE FUNCTION tracker_content_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF (SELECT active FROM tracker_space_admission_state WHERE singleton) OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) THEN
  IF TG_TABLE_NAME='projects' AND TG_OP='UPDATE' AND tracker_project_lifecycle_transition_allowed(to_jsonb(OLD),to_jsonb(NEW)) THEN RETURN NULL;END IF;
  IF TG_TABLE_NAME IN ('issues','issue_status_history') AND NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
   PERFORM tracker_authorize_native_issue_projection(TG_TABLE_NAME,TG_OP,to_jsonb(OLD),to_jsonb(NEW));
  ELSIF TG_TABLE_NAME IN ('comments','attachments') AND TG_OP='INSERT' AND NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
   PERFORM tracker_authorize_native_task_content(TG_TABLE_NAME,TG_OP,to_jsonb(NEW));
  ELSE PERFORM tracker_current_write_actor();END IF;
 END IF;RETURN NULL;
END $$;
