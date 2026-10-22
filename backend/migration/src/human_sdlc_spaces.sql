-- Human SDLC writes retain ordinary project-space and native-role barriers.
CREATE FUNCTION tracker_authorize_human_sdlc(table_name text,operation text,previous jsonb,desired jsonb) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE data jsonb; pid uuid; issue uuid; original jsonb; actual uuid;
BEGIN
 FOR data IN SELECT value FROM jsonb_array_elements(CASE operation WHEN 'INSERT' THEN jsonb_build_array(desired) WHEN 'DELETE' THEN jsonb_build_array(previous) ELSE jsonb_build_array(previous,desired) END) LOOP
  pid:=NULL;issue:=NULL;
  IF data ? 'project_id' THEN pid:=(data->>'project_id')::uuid;
  ELSIF data ? 'task_id' THEN issue:=(data->>'task_id')::uuid;
  ELSE RAISE EXCEPTION 'human_sdlc_resource_mapping_required' USING ERRCODE='42501';END IF;
  IF issue IS NOT NULL THEN
   SELECT to_jsonb(i) INTO original FROM issues i WHERE id=issue;
   IF original IS NULL THEN RAISE EXCEPTION 'human_sdlc_issue_missing' USING ERRCODE='42501';END IF;
   pid:=(original->>'project_id')::uuid;
  END IF;
  IF pid IS NULL THEN RAISE EXCEPTION 'human_sdlc_project_missing' USING ERRCODE='42501';END IF;
  -- Reuse the same locked project, active-space and current native-role contract.
  PERFORM tracker_authorize_project_content('boards','UPDATE',jsonb_build_object('project_id',pid),jsonb_build_object('project_id',pid));
  IF issue IS NOT NULL THEN
   SELECT project_id INTO actual FROM issues WHERE id=issue FOR SHARE;
   IF actual IS DISTINCT FROM pid THEN RAISE EXCEPTION 'human_sdlc_issue_moved' USING ERRCODE='40001';END IF;
  END IF;
 END LOOP;
END $$;

CREATE OR REPLACE FUNCTION tracker_namespace_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE data jsonb; pid uuid; pids uuid[] := '{}'; lifecycle text;
BEGIN
    PERFORM singleton FROM tracker_space_admission_state WHERE singleton FOR SHARE;
    IF NOT FOUND THEN RAISE EXCEPTION 'space_admission_state_missing' USING ERRCODE='55000';END IF;
    -- Only the dedicated, receipted project lifecycle transition may restore
    -- its independent archive metadata through an old archived 1:1 binding.
    IF TG_TABLE_NAME='projects' AND TG_OP='UPDATE' AND tracker_project_lifecycle_transition_allowed(to_jsonb(OLD),to_jsonb(NEW)) THEN RETURN NEW;END IF;
    IF (SELECT active FROM tracker_space_admission_state WHERE singleton) OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) THEN
        IF TG_TABLE_NAME='projects' AND TG_OP='UPDATE' THEN
         IF OLD.namespace_managed AND NOT NEW.namespace_managed THEN RAISE EXCEPTION 'namespace_marker_immutable' USING ERRCODE='42501';END IF;
        END IF;
        IF TG_TABLE_NAME=ANY(ARRAY['sdlc_tasks','sdlc_draft_creations','sdlc_answers','sdlc_evidence','sdlc_confirmations','sdlc_idempotency','sdlc_outbox','sdlc_requests','sdlc_project_routing_revisions','sdlc_project_routing_heads','sdlc_project_routing_operations','sdlc_task_routing_snapshots']) THEN
            PERFORM tracker_authorize_human_sdlc(TG_TABLE_NAME,TG_OP,to_jsonb(OLD),to_jsonb(NEW));
        ELSE
            PERFORM tracker_authorize_project_content(TG_TABLE_NAME,TG_OP,to_jsonb(OLD),to_jsonb(NEW));
        END IF;
        IF TG_OP='DELETE' THEN RETURN OLD;END IF;RETURN NEW;
    END IF;
    IF TG_TABLE_NAME='projects' AND TG_OP='UPDATE' THEN
        IF OLD.namespace_managed AND NOT NEW.namespace_managed THEN RAISE EXCEPTION 'namespace_marker_immutable' USING ERRCODE='42501'; END IF;
        IF NOT OLD.namespace_managed AND NEW.namespace_managed AND to_jsonb(OLD)-'namespace_managed'=to_jsonb(NEW)-'namespace_managed' AND EXISTS(SELECT 1 FROM tracker_namespace_bindings WHERE resource_id=NEW.id) THEN RETURN NEW; END IF;
    END IF;
    FOR data IN SELECT value FROM jsonb_array_elements(CASE TG_OP WHEN 'INSERT' THEN jsonb_build_array(to_jsonb(NEW)) WHEN 'DELETE' THEN jsonb_build_array(to_jsonb(OLD)) ELSE jsonb_build_array(to_jsonb(OLD),to_jsonb(NEW)) END)
    LOOP
        IF TG_TABLE_NAME='projects' THEN pids := array_append(pids,(data->>'id')::uuid);
        ELSIF data ? 'project_id' THEN pids := array_append(pids,(data->>'project_id')::uuid);
        ELSIF data ? 'task_id' THEN
            FOR pid IN SELECT project_id FROM issues WHERE id=(data->>'task_id')::uuid LOOP pids := array_append(pids,pid); END LOOP;
        ELSIF data ? 'execution_id' THEN
            FOR pid IN SELECT t.project_id FROM sdlc_tasks t JOIN sdlc_pm_executions e USING(task_id) WHERE e.execution_id=(data->>'execution_id')::uuid LOOP pids := array_append(pids,pid); END LOOP;
        ELSIF data ? 'assignment_id' THEN
            FOR pid IN SELECT t.project_id FROM sdlc_tasks t JOIN sdlc_analysis_reservations r USING(task_id) WHERE r.assignment_id=(data->>'assignment_id')::uuid LOOP pids := array_append(pids,pid); END LOOP;
        ELSE
            FOR pid IN SELECT project_id FROM issues WHERE id IN ((data->>'issue_id')::uuid,(data->>'source_id')::uuid,(data->>'target_id')::uuid) LOOP pids := array_append(pids,pid); END LOOP;
        END IF;
    END LOOP;
    IF EXISTS(SELECT 1 FROM projects p WHERE p.id=ANY(pids) AND p.namespace_managed AND NOT EXISTS(SELECT 1 FROM tracker_namespace_bindings b WHERE b.resource_id=p.id)) THEN
        RAISE EXCEPTION 'namespace_projection_missing' USING ERRCODE='42501';
    END IF;
    FOR lifecycle IN SELECT state FROM tracker_namespace_bindings WHERE resource_id=ANY(pids) ORDER BY resource_id FOR SHARE
    LOOP
        IF lifecycle <> 'active' OR (TG_TABLE_NAME='projects' AND TG_OP='DELETE') THEN
            RAISE EXCEPTION 'namespace_resource_read_only' USING ERRCODE='42501';
        END IF;
    END LOOP;
    IF TG_OP='DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END $$;
