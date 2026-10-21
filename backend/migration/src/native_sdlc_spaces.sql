-- A native product capability is independent of human space membership.
-- It is still bounded by its original verified clock, exact loaded aggregate,
-- existing native project ACL and the actual legacy/source namespace owners.
CREATE FUNCTION tracker_current_native_caller() RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE caller jsonb; validity jsonb; now_seconds bigint;
BEGIN
 BEGIN caller:=NULLIF(current_setting('pdlc.tracker.native_caller',true),'')::jsonb;
 EXCEPTION WHEN invalid_text_representation THEN RAISE EXCEPTION 'native_caller_invalid' USING ERRCODE='42501';END;
 validity:=caller->'validity';now_seconds:=floor(extract(epoch FROM clock_timestamp()))::bigint;
 IF caller IS NULL OR caller->>'schema_version' IS DISTINCT FROM '1'
  OR jsonb_typeof(caller->'identity') IS DISTINCT FROM 'object'
  OR (caller->'identity')-ARRAY['issuer','subject']<>'{}'::jsonb
  OR NOT ((caller->'identity') ?& ARRAY['issuer','subject'])
  OR NULLIF(btrim(caller->'identity'->>'issuer'),'') IS NULL
  OR NULLIF(btrim(caller->'identity'->>'subject'),'') IS NULL
  OR jsonb_typeof(caller->'scopes') IS DISTINCT FROM 'array'
  OR jsonb_typeof(validity->'observed_at') IS DISTINCT FROM 'number'
  OR jsonb_typeof(validity->'valid_until') IS DISTINCT FROM 'number'
  OR NOT coalesce(validity->>'observed_at' ~ '^[0-9]+$',false)
  OR NOT coalesce(validity->>'valid_until' ~ '^[0-9]+$',false)
 THEN RAISE EXCEPTION 'native_caller_invalid' USING ERRCODE='42501';END IF;
 IF (validity->>'observed_at')::bigint>now_seconds
  OR (validity->>'valid_until')::bigint<=now_seconds
  OR (validity->>'valid_until')::bigint>(validity->>'observed_at')::bigint+60
 THEN RAISE EXCEPTION 'native_request_authority_expired' USING ERRCODE='42501';END IF;
 RETURN caller;
END $$;
CREATE FUNCTION tracker_bind_native_task(scope jsonb) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE caller jsonb; aggregate record; profile record; owner record; entry jsonb;
 spaces jsonb:='[]'; state text; deadline bigint; mutation boolean;
BEGIN
 caller:=tracker_current_native_caller();mutation:=caller->'writable'='true'::jsonb;
 IF scope IS NULL OR scope->>'schema_version' IS DISTINCT FROM '1'
  OR scope-ARRAY['schema_version','task_id','project_id','identity','validity','writable']<>'{}'::jsonb
  OR NOT(scope ?& ARRAY['schema_version','task_id','project_id','identity','validity','writable'])
  OR scope->'identity' IS DISTINCT FROM caller->'identity'
  OR scope->'validity' IS DISTINCT FROM caller->'validity'
  OR scope->'writable' IS DISTINCT FROM caller->'writable'
 THEN RAISE EXCEPTION 'native_task_scope_invalid' USING ERRCODE='42501';END IF;
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
CREATE FUNCTION tracker_sdlc_row_task(data jsonb) RETURNS uuid LANGUAGE plpgsql VOLATILE AS $$
DECLARE result uuid;
BEGIN
 IF data->>'task_id' IS NOT NULL THEN RETURN (data->>'task_id')::uuid;
 ELSIF data->>'root_task_id' IS NOT NULL THEN RETURN (data->>'root_task_id')::uuid;
 ELSIF data->>'execution_id' IS NOT NULL THEN
  SELECT task_id INTO result FROM sdlc_pm_executions WHERE execution_id=(data->>'execution_id')::uuid FOR SHARE;
 ELSIF data->>'assignment_id' IS NOT NULL THEN
  SELECT task_id INTO result FROM sdlc_analysis_reservations WHERE assignment_id=(data->>'assignment_id')::uuid FOR SHARE;
  IF result IS NULL THEN SELECT task_id INTO result FROM sdlc_pm_executions WHERE assignment_id=(data->>'assignment_id')::uuid FOR SHARE;END IF;
 END IF;
 RETURN result;
END $$;
CREATE FUNCTION tracker_authorize_native_sdlc(table_name text,operation text,previous jsonb,desired jsonb) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE caller jsonb; scope jsonb; data jsonb; task uuid; project uuid;
BEGIN
 caller:=tracker_current_native_caller();
 BEGIN scope:=NULLIF(current_setting('pdlc.tracker.native_task_scope',true),'')::jsonb;
 EXCEPTION WHEN invalid_text_representation THEN RAISE EXCEPTION 'native_task_scope_invalid' USING ERRCODE='42501';END;
 IF scope IS NULL OR scope->'identity' IS DISTINCT FROM caller->'identity'
  OR scope->'validity' IS DISTINCT FROM caller->'validity'
  OR scope->'writable' IS DISTINCT FROM 'true'::jsonb OR caller->'writable' IS DISTINCT FROM 'true'::jsonb
 THEN RAISE EXCEPTION 'native_task_scope_required' USING ERRCODE='42501';END IF;
 -- These existing protocol tables are task-owned. Project-wide routing and
 -- source policy administration cannot borrow a task capability.
 IF table_name<>ALL(ARRAY['sdlc_tasks','sdlc_agent_bindings','sdlc_assignments','sdlc_pm_executions','sdlc_pm_execution_leases',
  'sdlc_pm_lease_operations','sdlc_requests','sdlc_question_versions','sdlc_options','sdlc_requirements','sdlc_answers',
  'sdlc_evidence','sdlc_confirmations','sdlc_task_routing_snapshots','sdlc_analysis_intents','sdlc_analysis_reservations',
  'sdlc_analysis_reservation_leases','sdlc_analysis_reservation_operations','sdlc_idempotency','sdlc_outbox',
  'sdlc_task_source_selections','sdlc_draft_source_enrollments','sdlc_source_ref_owners','sdlc_task_source_bindings'])
 THEN RAISE EXCEPTION 'native_task_operation_not_allowed' USING ERRCODE='42501';END IF;
 FOR data IN SELECT value FROM jsonb_array_elements(CASE operation WHEN 'INSERT' THEN jsonb_build_array(desired) WHEN 'DELETE' THEN jsonb_build_array(previous) ELSE jsonb_build_array(previous,desired) END) LOOP
  task:=tracker_sdlc_row_task(data);
  IF task IS NULL OR task IS DISTINCT FROM (scope->>'task_id')::uuid THEN RAISE EXCEPTION 'native_foreign_task' USING ERRCODE='42501';END IF;
  SELECT project_id INTO project FROM issues WHERE id=task AND deleted_at IS NULL FOR SHARE;
  IF project IS DISTINCT FROM (scope->>'project_id')::uuid OR (data->>'project_id' IS NOT NULL AND (data->>'project_id')::uuid IS DISTINCT FROM project)
  THEN RAISE EXCEPTION 'native_foreign_project' USING ERRCODE='42501';END IF;
 END LOOP;
 -- Also catches time spent behind an aggregate lock since the scope was bound.
 PERFORM tracker_bind_native_task(scope-'spaces');
END $$;
CREATE OR REPLACE FUNCTION tracker_authorize_human_sdlc(table_name text,operation text,previous jsonb,desired jsonb) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE data jsonb; pid uuid; issue uuid; original jsonb; actual uuid;
BEGIN
 FOR data IN SELECT value FROM jsonb_array_elements(CASE operation WHEN 'INSERT' THEN jsonb_build_array(desired) WHEN 'DELETE' THEN jsonb_build_array(previous) ELSE jsonb_build_array(previous,desired) END) LOOP
  pid:=NULL;issue:=tracker_sdlc_row_task(data);
  IF data->>'project_id' IS NOT NULL THEN pid:=(data->>'project_id')::uuid;END IF;
  IF issue IS NOT NULL THEN
   SELECT to_jsonb(i) INTO original FROM issues i WHERE id=issue;
   IF original IS NULL THEN RAISE EXCEPTION 'human_sdlc_issue_missing' USING ERRCODE='42501';END IF;
   IF pid IS NOT NULL AND pid IS DISTINCT FROM (original->>'project_id')::uuid THEN RAISE EXCEPTION 'human_sdlc_project_changed' USING ERRCODE='42501';END IF;
   pid:=(original->>'project_id')::uuid;
  END IF;
  IF pid IS NULL THEN RAISE EXCEPTION 'human_sdlc_project_missing' USING ERRCODE='42501';END IF;
  PERFORM tracker_authorize_project_content('boards','UPDATE',jsonb_build_object('project_id',pid),jsonb_build_object('project_id',pid));
  IF issue IS NOT NULL THEN
   SELECT project_id INTO actual FROM issues WHERE id=issue FOR SHARE;
   IF actual IS DISTINCT FROM pid THEN RAISE EXCEPTION 'human_sdlc_issue_moved' USING ERRCODE='40001';END IF;
  END IF;
 END LOOP;
END $$;

CREATE FUNCTION tracker_authorize_native_issue_projection(table_name text,operation text,previous jsonb,desired jsonb) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE scope jsonb; projection jsonb; caller jsonb; task uuid; subject uuid;
BEGIN
 caller:=tracker_current_native_caller();
 BEGIN
  scope:=NULLIF(current_setting('pdlc.tracker.native_task_scope',true),'')::jsonb;
  projection:=NULLIF(current_setting('pdlc.tracker.native_issue_projection',true),'')::jsonb;
 EXCEPTION WHEN invalid_text_representation THEN RAISE EXCEPTION 'native_projection_invalid' USING ERRCODE='42501';END;
 IF scope IS NULL OR projection IS NULL OR projection->>'task_id' IS DISTINCT FROM scope->>'task_id'
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
        IF TG_TABLE_NAME LIKE 'sdlc_%' THEN
            IF NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
                PERFORM tracker_authorize_native_sdlc(TG_TABLE_NAME,TG_OP,to_jsonb(OLD),to_jsonb(NEW));
            ELSE
                PERFORM tracker_authorize_human_sdlc(TG_TABLE_NAME,TG_OP,to_jsonb(OLD),to_jsonb(NEW));
            END IF;
        ELSIF TG_TABLE_NAME IN ('issues','issue_status_history') AND NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
            PERFORM tracker_authorize_native_issue_projection(TG_TABLE_NAME,TG_OP,to_jsonb(OLD),to_jsonb(NEW));
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

-- Every protected SDLC write keeps its original request window through COMMIT.
CREATE FUNCTION tracker_sdlc_commit_clock() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF (SELECT active FROM tracker_space_admission_state WHERE singleton) OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) THEN
  IF NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN PERFORM tracker_current_native_caller();
  ELSE PERFORM tracker_current_write_actor();END IF;
 END IF;
 RETURN NULL;
END $$;
DO $$ DECLARE item record;BEGIN
 FOR item IN SELECT DISTINCT c.oid::regclass AS table_name FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid JOIN pg_namespace n ON n.oid=c.relnamespace
  WHERE n.nspname=current_schema() AND c.relname LIKE 'sdlc_%' AND t.tgname='namespace_write_guard'
 LOOP
  EXECUTE format('CREATE CONSTRAINT TRIGGER shared_sdlc_commit_clock AFTER INSERT OR UPDATE OR DELETE ON %s DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_sdlc_commit_clock()',item.table_name);
 END LOOP;
END $$;

-- The native projection port above cannot lend authority to other content rows.
CREATE OR REPLACE FUNCTION tracker_content_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF (SELECT active FROM tracker_space_admission_state WHERE singleton) OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) THEN
  IF TG_TABLE_NAME='projects' AND TG_OP='UPDATE' AND tracker_project_lifecycle_transition_allowed(to_jsonb(OLD),to_jsonb(NEW)) THEN RETURN NULL;END IF;
  IF TG_TABLE_NAME IN ('issues','issue_status_history') AND NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
   PERFORM tracker_authorize_native_issue_projection(TG_TABLE_NAME,TG_OP,to_jsonb(OLD),to_jsonb(NEW));
  ELSE PERFORM tracker_current_write_actor();END IF;
 END IF;RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION tracker_execution_start_space_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task uuid; project uuid; assignment uuid; scope jsonb; required jsonb; actor jsonb;
    deadline bigint; current_time_seconds bigint; entry jsonb; candidate record; origin text; assignment_revision bigint;
    enforced boolean; matched boolean:=false; required_matched boolean:=false; affected integer;
    human jsonb; native jsonb; original_scope jsonb; source_owner jsonb;
BEGIN
    IF TG_TABLE_NAME='sdlc_assignments' THEN task:=NEW.task_id;origin:='pm';assignment_revision:=NEW.version;
    ELSE task:=NEW.root_task_id;origin:='analysis';assignment_revision:=NULL; END IF;
    SELECT project_id INTO project FROM sdlc_tasks WHERE task_id=task;
    -- Same project-first order as link/lifecycle writes. Archival is not terminal
    -- evidence; completion/control receipts are not execution-start mutations.
    PERFORM id FROM projects WHERE id=project FOR SHARE;
    IF EXISTS(SELECT 1 FROM projects WHERE id=project AND archived_at IS NOT NULL) THEN RAISE EXCEPTION 'project_archived_read_only' USING ERRCODE='42501';END IF;
    PERFORM singleton FROM tracker_space_admission_state WHERE singleton FOR SHARE;
    enforced:=(SELECT active FROM tracker_space_admission_state WHERE singleton);
    -- Lock first even during expand/compatibility mode. A concurrent first v2
    -- catalog update must not be skipped using an earlier "legacy" snapshot.
    FOR candidate IN SELECT c.* FROM tracker_project_spaces l JOIN tracker_space_catalog c USING(registry_instance_id,space_id)
        WHERE l.project_id=project ORDER BY c.registry_instance_id,c.space_id FOR SHARE OF c
    LOOP
        IF candidate.metadata IS NOT NULL THEN enforced:=true; END IF;
    END LOOP;
    IF NOT enforced THEN RETURN NEW; END IF;
    BEGIN
        scope:=NULLIF(current_setting('pdlc.tracker.execution_spaces',true),'')::jsonb;
        required:=NULLIF(current_setting('pdlc.tracker.execution_required_wiki_space',true),'')::jsonb;
        actor:=NULLIF(current_setting('pdlc.tracker.execution_actor',true),'')::jsonb;
        deadline:=NULLIF(current_setting('pdlc.tracker.admission_valid_until',true),'')::bigint;
    EXCEPTION WHEN invalid_text_representation OR numeric_value_out_of_range THEN
        RAISE EXCEPTION 'invalid_execution_space_grant' USING ERRCODE='42501';
    END;

    IF scope IS NULL THEN
        -- Freeze only the actual project's admitted links and exact repository
        -- owner. Human account-wide memberships are not an execution footprint.
        IF NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
            native:=tracker_current_native_caller();
            original_scope:=NULLIF(current_setting('pdlc.tracker.native_task_scope',true),'')::jsonb;
            IF original_scope->>'task_id' IS DISTINCT FROM task::text OR original_scope->>'project_id' IS DISTINCT FROM project::text
                OR original_scope->'identity' IS DISTINCT FROM native->'identity' OR native->'writable' IS DISTINCT FROM 'true'::jsonb
            THEN RAISE EXCEPTION 'native_execution_task_scope_required' USING ERRCODE='42501';END IF;
            actor:=native->'identity';deadline:=(native->'validity'->>'valid_until')::bigint;
            scope:=original_scope->'spaces';
            SELECT scope||coalesce(jsonb_agg(jsonb_build_object('registry_instance_id',c.registry_instance_id,'space_id',c.space_id))
                FILTER(WHERE NOT scope @> jsonb_build_array(jsonb_build_object('registry_instance_id',c.registry_instance_id,'space_id',c.space_id))),'[]'::jsonb)
                INTO scope FROM tracker_project_spaces l JOIN tracker_space_catalog c USING(registry_instance_id,space_id)
                WHERE l.project_id=project AND c.admin_revision IS NOT NULL AND c.state='active';
        ELSE
            human:=tracker_current_write_actor();
            actor:=human->'identity';deadline:=(human->>'valid_until')::bigint;
            SELECT coalesce(jsonb_agg(jsonb_build_object('registry_instance_id',c.registry_instance_id,'space_id',c.space_id)),'[]'::jsonb)
                INTO scope FROM tracker_project_spaces l JOIN tracker_space_catalog c USING(registry_instance_id,space_id)
                WHERE l.project_id=project AND c.admin_revision IS NOT NULL AND c.state='active'
                    AND human->'active_spaces' @> jsonb_build_array(jsonb_build_object('registry_instance_id',c.registry_instance_id,'space_id',c.space_id));
            SELECT r.payload->'verification'->'namespace' INTO source_owner
                FROM sdlc_effective_task_source_selections selected JOIN sdlc_source_policy_revisions r
                    ON r.project_id=selected.project_id AND r.source_revision=selected.source_revision
                WHERE selected.task_id=task AND selected.project_id=project;
            IF source_owner IS NOT NULL THEN
                source_owner:=jsonb_build_object('registry_instance_id',source_owner->>'registry_instance_id','space_id',source_owner->>'namespace_id');
                IF NOT tracker_valid_space_ref(source_owner) OR NOT human->'active_spaces' @> jsonb_build_array(source_owner) THEN
                    RAISE EXCEPTION 'execution_repository_owner_not_admitted' USING ERRCODE='42501';
                END IF;
                IF NOT scope @> jsonb_build_array(source_owner) THEN scope:=scope||jsonb_build_array(source_owner);END IF;
            END IF;
        END IF;
        -- This protocol carries no confirmed Wiki owner. A Forge namespace is
        -- never relabelled as a Wiki space.
        required:=NULL;
    END IF;
    current_time_seconds:=floor(extract(epoch FROM clock_timestamp()))::bigint;
    IF scope IS NULL OR jsonb_typeof(scope) IS DISTINCT FROM 'array' THEN RAISE EXCEPTION 'execution_space_grant_required' USING ERRCODE='42501'; END IF;
    IF jsonb_array_length(scope) NOT BETWEEN 1 AND 1000 OR deadline IS NULL
        OR deadline<=current_time_seconds OR deadline>current_time_seconds+60
        OR actor IS NULL OR jsonb_typeof(actor) IS DISTINCT FROM 'object'
        OR jsonb_typeof(actor->'issuer') IS DISTINCT FROM 'string' OR jsonb_typeof(actor->'subject') IS DISTINCT FROM 'string'
        OR actor-ARRAY['issuer','subject']<>'{}'::jsonb
        OR NOT actor ?& ARRAY['issuer','subject']
        OR NULLIF(btrim(actor->>'issuer'),'') IS NULL OR NULLIF(btrim(actor->>'subject'),'') IS NULL
    THEN RAISE EXCEPTION 'invalid_execution_space_grant' USING ERRCODE='42501'; END IF;
    FOR entry IN SELECT value FROM jsonb_array_elements(scope) LOOP
        IF NOT tracker_valid_space_ref(entry) THEN RAISE EXCEPTION 'invalid_execution_space_grant' USING ERRCODE='42501'; END IF;
    END LOOP;
    IF (SELECT count(DISTINCT value) FROM jsonb_array_elements(scope))<>jsonb_array_length(scope) THEN RAISE EXCEPTION 'duplicate_execution_spaces' USING ERRCODE='42501'; END IF;
    IF required IS NOT NULL AND required<>'null'::jsonb AND (NOT tracker_valid_space_ref(required) OR NOT scope @> jsonb_build_array(required)) THEN
        RAISE EXCEPTION 'wiki_space_outside_execution_grant' USING ERRCODE='42501';
    END IF;
    -- Same catalog lock as project-link commit and the owner archive barrier.
    FOR candidate IN SELECT c.* FROM tracker_project_spaces l JOIN tracker_space_catalog c USING(registry_instance_id,space_id)
        WHERE l.project_id=project ORDER BY c.registry_instance_id,c.space_id FOR SHARE OF c
    LOOP
        IF candidate.state='active' AND candidate.admin_revision IS NOT NULL
            AND scope @> jsonb_build_array(jsonb_build_object('registry_instance_id',candidate.registry_instance_id,'space_id',candidate.space_id)) THEN
            matched:=true;
            IF required IS NOT NULL AND required=jsonb_build_object('registry_instance_id',candidate.registry_instance_id,'space_id',candidate.space_id) THEN required_matched:=true; END IF;
        END IF;
    END LOOP;
    FOR entry IN SELECT value FROM jsonb_array_elements(scope) LOOP
        PERFORM c.space_id FROM tracker_space_catalog c WHERE c.registry_instance_id=(entry->>'registry_instance_id')::uuid
            AND c.space_id=(entry->>'space_id')::uuid AND c.admin_revision IS NOT NULL AND c.state='active' FOR SHARE;
        IF NOT FOUND THEN RAISE EXCEPTION 'execution_resource_space_not_active' USING ERRCODE='42501';END IF;
    END LOOP;
    IF NOT matched OR (required IS NOT NULL AND required<>'null'::jsonb AND NOT required_matched) THEN
        RAISE EXCEPTION 'execution_space_not_active_or_admitted' USING ERRCODE='42501';
    END IF;
    IF floor(extract(epoch FROM clock_timestamp()))::bigint>=deadline THEN RAISE EXCEPTION 'execution_admission_expired_after_wait' USING ERRCODE='42501'; END IF;
    IF TG_TABLE_NAME='sdlc_assignments' THEN
        BEGIN assignment:=(NEW.payload->>'assignment_id')::uuid;
        EXCEPTION WHEN invalid_text_representation THEN RAISE EXCEPTION 'assignment_identity_required' USING ERRCODE='42501'; END;
    ELSE assignment:=NEW.assignment_id; END IF;
    IF assignment IS NULL OR assignment='00000000-0000-0000-0000-000000000000' THEN RAISE EXCEPTION 'assignment_identity_required' USING ERRCODE='42501'; END IF;
    INSERT INTO tracker_execution_space_grants AS original(assignment_id,task_id,project_id,origin,assignment_version,spaces,required_wiki_space,original_actor,initial_valid_until)
        VALUES(assignment,task,project,origin,assignment_revision,scope,NULLIF(required,'null'::jsonb),actor,deadline)
        ON CONFLICT(assignment_id) DO UPDATE SET assignment_id=EXCLUDED.assignment_id
        WHERE original.task_id=EXCLUDED.task_id AND original.project_id=EXCLUDED.project_id
            AND original.origin=EXCLUDED.origin AND original.assignment_version IS NOT DISTINCT FROM EXCLUDED.assignment_version
            AND original.spaces @> EXCLUDED.spaces AND EXCLUDED.spaces @> original.spaces
            AND original.required_wiki_space IS NOT DISTINCT FROM EXCLUDED.required_wiki_space
            AND original.original_actor=EXCLUDED.original_actor;
    GET DIAGNOSTICS affected=ROW_COUNT;
    IF affected<>1 THEN RAISE EXCEPTION 'execution_space_grant_rebinding_forbidden' USING ERRCODE='42501'; END IF;
    RETURN NEW;
END $$;
