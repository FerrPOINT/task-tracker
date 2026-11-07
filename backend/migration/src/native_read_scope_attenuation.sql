-- A mutating request may inspect retained history without lending write authority.
-- Identity and the original validity window stay exact; read cannot become write.
CREATE OR REPLACE FUNCTION tracker_bind_native_task(scope jsonb) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE caller jsonb; aggregate record; profile record; owner record; entry jsonb;
 spaces jsonb:='[]'; state text; deadline bigint; mutation boolean;
BEGIN
 caller:=tracker_current_native_caller();
 IF scope IS NULL OR scope->>'schema_version' IS DISTINCT FROM '1'
  OR scope-ARRAY['schema_version','task_id','project_id','identity','validity','writable']<>'{}'::jsonb
  OR NOT(scope ?& ARRAY['schema_version','task_id','project_id','identity','validity','writable'])
  OR scope->'identity' IS DISTINCT FROM caller->'identity'
  OR scope->'validity' IS DISTINCT FROM caller->'validity'
  OR jsonb_typeof(scope->'writable') IS DISTINCT FROM 'boolean'
  OR jsonb_typeof(caller->'writable') IS DISTINCT FROM 'boolean'
  OR (scope->'writable'='true'::jsonb AND caller->'writable'<>'true'::jsonb)
 THEN RAISE EXCEPTION 'native_task_scope_invalid' USING ERRCODE='42501';END IF;
 mutation:=scope->'writable'='true'::jsonb;
 SELECT t.task_id,t.project_id,i.project_id AS issue_project,p.owner_id,p.archived_at INTO aggregate
  FROM sdlc_tasks t JOIN issues i ON i.id=t.task_id AND i.deleted_at IS NULL JOIN projects p ON p.id=t.project_id
  WHERE t.task_id=(scope->>'task_id')::uuid FOR SHARE OF t,i,p;
 IF NOT FOUND OR aggregate.project_id IS DISTINCT FROM (scope->>'project_id')::uuid
  OR aggregate.issue_project IS DISTINCT FROM aggregate.project_id
  OR (mutation AND aggregate.archived_at IS NOT NULL)
 THEN RAISE EXCEPTION 'native_task_owner_changed' USING ERRCODE='42501';END IF;
 SELECT id INTO profile FROM users WHERE central_sub=caller->'identity'->>'subject' AND is_active FOR SHARE;
 IF NOT FOUND THEN RAISE EXCEPTION 'native_profile_not_confirmed' USING ERRCODE='42501';END IF;
 IF profile.id<>aggregate.owner_id THEN
  PERFORM user_id FROM project_members WHERE project_id=aggregate.project_id AND user_id=profile.id FOR SHARE;
  IF NOT FOUND THEN RAISE EXCEPTION 'native_project_role_required' USING ERRCODE='42501';END IF;
 END IF;
 -- The retained 1:1 owner is explicit. Never choose the first M:N project link.
 SELECT registry_instance_id,namespace_id INTO owner FROM tracker_namespace_bindings WHERE resource_id=aggregate.project_id FOR SHARE;
 IF NOT FOUND THEN RAISE EXCEPTION 'native_namespace_binding_required' USING ERRCODE='42501';END IF;
 spaces:=jsonb_build_array(jsonb_build_object('registry_instance_id',owner.registry_instance_id,'space_id',owner.namespace_id));
 SELECT payload->'namespace' INTO entry FROM sdlc_task_source_bindings WHERE task_id=aggregate.task_id ORDER BY requirement_revision DESC LIMIT 1;
 IF entry IS NOT NULL THEN
  entry:=jsonb_build_object('registry_instance_id',entry->>'registry_instance_id','space_id',entry->>'namespace_id');
  IF NOT tracker_valid_space_ref(entry) THEN RAISE EXCEPTION 'native_source_namespace_invalid' USING ERRCODE='42501';END IF;
  IF NOT spaces @> jsonb_build_array(entry) THEN spaces:=spaces||jsonb_build_array(entry);END IF;
 END IF;
 FOR entry IN SELECT value FROM jsonb_array_elements(spaces) LOOP
  SELECT c.state INTO state FROM tracker_space_catalog c
   WHERE c.registry_instance_id=(entry->>'registry_instance_id')::uuid AND c.space_id=(entry->>'space_id')::uuid AND c.admin_revision IS NOT NULL FOR SHARE;
  IF NOT FOUND OR state NOT IN ('active','archiving','archived')
   OR (mutation AND state='archived')
   OR (mutation AND state='archiving' AND NOT EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=aggregate.task_id))
  THEN RAISE EXCEPTION 'native_resource_space_not_admitted' USING ERRCODE='42501';END IF;
 END LOOP;
 -- Re-check the original clock after every blocking lock, without renewing it.
 PERFORM tracker_current_native_caller();
 PERFORM set_config('pdlc.tracker.native_task_scope',(scope||jsonb_build_object('spaces',spaces))::text,true);
END $$;

-- Status projections are writes even when the original request also read history.
CREATE OR REPLACE FUNCTION tracker_authorize_native_issue_projection(table_name text,operation text,previous jsonb,desired jsonb) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE scope jsonb; projection jsonb; caller jsonb; task uuid; subject uuid;
BEGIN
 caller:=tracker_current_native_caller();
 BEGIN
  scope:=NULLIF(current_setting('pdlc.tracker.native_task_scope',true),'')::jsonb;
  projection:=NULLIF(current_setting('pdlc.tracker.native_issue_projection',true),'')::jsonb;
 EXCEPTION WHEN invalid_text_representation THEN RAISE EXCEPTION 'native_projection_invalid' USING ERRCODE='42501';END;
 IF scope IS NULL OR scope->'writable' IS DISTINCT FROM 'true'::jsonb
  OR caller->'writable' IS DISTINCT FROM 'true'::jsonb
  OR projection IS NULL OR projection->>'task_id' IS DISTINCT FROM scope->>'task_id'
  OR projection-ARRAY['task_id','status_id']<>'{}'::jsonb
  OR NOT(projection ?& ARRAY['task_id','status_id'])
 THEN RAISE EXCEPTION 'native_projection_required' USING ERRCODE='42501';END IF;
 task:=(scope->>'task_id')::uuid;
 PERFORM tracker_bind_native_task(scope-'spaces');
 IF table_name='issues' AND operation='UPDATE' THEN
  IF previous->>'id' IS DISTINCT FROM task::text OR desired->>'id' IS DISTINCT FROM task::text
   OR previous-ARRAY['status_id','sprint_id','updated_at'] IS DISTINCT FROM desired-ARRAY['status_id','sprint_id','updated_at']
   OR desired->>'status_id' IS DISTINCT FROM projection->>'status_id'
   OR desired->'sprint_id' IS DISTINCT FROM 'null'::jsonb
  THEN RAISE EXCEPTION 'native_projection_content_changed' USING ERRCODE='42501';END IF;
 ELSIF table_name='issue_status_history' AND operation='INSERT' THEN
  SELECT id INTO subject FROM users WHERE central_sub=caller->'identity'->>'subject' AND is_active FOR SHARE;
  IF subject IS NULL OR desired->>'issue_id' IS DISTINCT FROM task::text
   OR desired->>'to_status_id' IS DISTINCT FROM projection->>'status_id'
   OR desired->>'changed_by_id' IS DISTINCT FROM subject::text
  THEN RAISE EXCEPTION 'native_projection_history_changed' USING ERRCODE='42501';END IF;
 ELSE RAISE EXCEPTION 'native_projection_operation_denied' USING ERRCODE='42501';END IF;
END $$;
