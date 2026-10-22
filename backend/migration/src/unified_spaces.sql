-- Canonical links are owned by Tracker. Legacy receipts remain unchanged.
-- Enforcement stays disabled until the full compatible cohort has been qualified.
-- Independent project archive; no Space owner command writes these columns.
ALTER TABLE projects ADD COLUMN archived_at timestamptz;
ALTER TABLE projects ADD COLUMN project_lifecycle_revision bigint NOT NULL DEFAULT 1 CHECK(project_lifecycle_revision>0);
CREATE TABLE tracker_space_admission_state (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    active boolean NOT NULL DEFAULT false
);
INSERT INTO tracker_space_admission_state(singleton, active) VALUES (true, false);
CREATE TABLE tracker_space_catalog (
    registry_instance_id uuid NOT NULL,
    space_id uuid NOT NULL,
    admin_revision bigint CHECK (admin_revision > 0),
    state text NOT NULL CHECK (state IN ('provisioning','active','archiving','archived','restoring')),
    operation_id uuid,
    PRIMARY KEY (registry_instance_id, space_id),
    CHECK (registry_instance_id <> '00000000-0000-0000-0000-000000000000'),
    CHECK (space_id <> '00000000-0000-0000-0000-000000000000')
);
CREATE TABLE tracker_project_space_sets (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0)
);
INSERT INTO tracker_project_space_sets(project_id) SELECT id FROM projects;
CREATE TABLE tracker_project_spaces (
    project_id uuid NOT NULL REFERENCES tracker_project_space_sets(project_id) ON DELETE RESTRICT,
    registry_instance_id uuid NOT NULL,
    space_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, registry_instance_id, space_id),
    FOREIGN KEY (registry_instance_id, space_id) REFERENCES tracker_space_catalog(registry_instance_id, space_id)
);
CREATE INDEX tracker_project_spaces_space ON tracker_project_spaces(registry_instance_id, space_id, project_id);
-- A legacy owner generation is not an Admin catalog revision: fresh owner confirmation is required.
INSERT INTO tracker_space_catalog(registry_instance_id,space_id,state,operation_id)
SELECT registry_instance_id,namespace_id,state,(command->>'operation_id')::uuid
FROM tracker_namespace_bindings;
INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id)
SELECT resource_id,registry_instance_id,namespace_id FROM tracker_namespace_bindings;

CREATE FUNCTION tracker_project_space_set_created() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO tracker_project_space_sets(project_id) VALUES (NEW.id);
    RETURN NEW;
END;
$$;
CREATE TRIGGER tracker_project_space_set_created AFTER INSERT ON projects
    FOR EACH ROW EXECUTE FUNCTION tracker_project_space_set_created();

CREATE FUNCTION tracker_project_spaces_fence() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE project uuid; candidates uuid[]; lifecycle text; catalog_revision bigint; enforcing boolean;
BEGIN
    IF TG_OP = 'INSERT' THEN candidates := ARRAY[NEW.project_id];
    ELSIF TG_OP = 'DELETE' THEN candidates := ARRAY[OLD.project_id];
    ELSE candidates := ARRAY[OLD.project_id, NEW.project_id]; END IF;
    FOR project IN SELECT DISTINCT id FROM unnest(candidates) id ORDER BY id LOOP
        PERFORM id FROM projects WHERE id=project FOR SHARE;
        IF EXISTS(SELECT 1 FROM projects WHERE id=project AND archived_at IS NOT NULL) THEN RAISE EXCEPTION 'project_archived_read_only' USING ERRCODE='42501';END IF;
        -- A real row update fences REPEATABLE READ too; a SELECT lock alone does not.
        UPDATE tracker_project_space_sets SET revision=revision+1 WHERE project_id=project;
        IF NOT FOUND THEN RAISE EXCEPTION 'project_space_set_missing' USING ERRCODE='23503'; END IF;
    END LOOP;
    SELECT active INTO enforcing FROM tracker_space_admission_state WHERE singleton;
    IF enforcing AND TG_OP <> 'DELETE' THEN
        SELECT state, admin_revision INTO lifecycle,catalog_revision
        FROM tracker_space_catalog WHERE registry_instance_id=NEW.registry_instance_id AND space_id=NEW.space_id FOR SHARE;
        IF catalog_revision IS NULL OR lifecycle IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'space_not_active_or_confirmed' USING ERRCODE='42501';
        END IF;
    END IF;
    IF TG_OP='DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER tracker_project_spaces_fence BEFORE INSERT OR UPDATE OR DELETE ON tracker_project_spaces
    FOR EACH ROW EXECUTE FUNCTION tracker_project_spaces_fence();

CREATE FUNCTION tracker_project_requires_space() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE project uuid; candidates uuid[];
BEGIN
    IF NOT (SELECT active FROM tracker_space_admission_state WHERE singleton) THEN RETURN NULL; END IF;
    IF TG_TABLE_NAME = 'projects' THEN candidates := ARRAY[NEW.id];
    ELSIF TG_OP = 'INSERT' THEN candidates := ARRAY[NEW.project_id];
    ELSIF TG_OP = 'DELETE' THEN candidates := ARRAY[OLD.project_id];
    ELSE candidates := ARRAY[OLD.project_id, NEW.project_id]; END IF;
    FOREACH project IN ARRAY candidates LOOP
        IF EXISTS (SELECT 1 FROM projects WHERE id=project)
            AND NOT EXISTS (SELECT 1 FROM tracker_project_spaces WHERE project_id=project) THEN
            RAISE EXCEPTION 'project_requires_space' USING ERRCODE='23514';
        END IF;
    END LOOP;
    RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER tracker_project_requires_space_after_create AFTER INSERT ON projects
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_project_requires_space();
CREATE CONSTRAINT TRIGGER tracker_project_requires_space_after_links AFTER INSERT OR UPDATE OR DELETE ON tracker_project_spaces
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_project_requires_space();

CREATE FUNCTION tracker_space_activation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.active AND NOT NEW.active THEN
        RAISE EXCEPTION 'space_admission_cannot_be_disabled' USING ERRCODE='42501';
    END IF;
    IF NEW.active AND NOT OLD.active THEN
        IF current_setting('transaction_isolation') <> 'read committed' THEN
            RAISE EXCEPTION 'space_activation_requires_read_committed' USING ERRCODE='25000';
        END IF;
        -- Finish in-flight legacy writes before validating the complete activation snapshot.
        LOCK TABLE projects, tracker_project_space_sets, tracker_project_spaces, tracker_space_catalog IN SHARE ROW EXCLUSIVE MODE;
        IF EXISTS (SELECT 1 FROM projects p WHERE NOT EXISTS (SELECT 1 FROM tracker_project_spaces l WHERE l.project_id=p.id)) THEN
            RAISE EXCEPTION 'unlinked_projects_prevent_activation' USING ERRCODE='23514';
        END IF;
        IF EXISTS (SELECT 1 FROM tracker_project_spaces l JOIN tracker_space_catalog s USING(registry_instance_id,space_id) WHERE s.admin_revision IS NULL) THEN
            RAISE EXCEPTION 'unconfirmed_spaces_prevent_activation' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER tracker_space_activation_guard BEFORE UPDATE ON tracker_space_admission_state
    FOR EACH ROW EXECUTE FUNCTION tracker_space_activation_guard();


ALTER TABLE tracker_space_catalog ADD COLUMN metadata jsonb;
CREATE TABLE tracker_space_owner_operations (
    operation_id uuid PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
    registry_instance_id uuid NOT NULL,
    space_id uuid NOT NULL,
    revision bigint NOT NULL CHECK(revision>0),
    command jsonb NOT NULL,
    readback jsonb NOT NULL,
    owner_issuer text NOT NULL,
    owner_subject text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(registry_instance_id,space_id,revision),
    FOREIGN KEY(registry_instance_id,space_id) REFERENCES tracker_space_catalog(registry_instance_id,space_id) ON DELETE RESTRICT
);
-- Immutable execution dependency, separate from renewable admission and process liveness.
-- Expiry of the initial decision must never be interpreted as a stopped execution.
CREATE TABLE tracker_execution_space_grants (
    assignment_id uuid PRIMARY KEY CHECK(assignment_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    origin text NOT NULL CHECK(origin IN ('pm','analysis')),
    assignment_version bigint,
    CHECK((origin='pm' AND assignment_version IS NOT NULL AND assignment_version>0) OR (origin='analysis' AND assignment_version IS NULL)),
    spaces jsonb NOT NULL CHECK(jsonb_typeof(spaces)='array' AND jsonb_array_length(spaces) BETWEEN 1 AND 1000),
    required_wiki_space jsonb,
    original_actor jsonb NOT NULL,
    initial_valid_until bigint NOT NULL CHECK(initial_valid_until>0),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE FUNCTION tracker_valid_space_ref(value jsonb) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
 SELECT coalesce(jsonb_typeof(value)='object'
    AND value-ARRAY['registry_instance_id','space_id']='{}'::jsonb
    AND value ?& ARRAY['registry_instance_id','space_id']
    AND value->>'registry_instance_id' ~ '^[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}$'
    AND value->>'space_id' ~ '^[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}$'
    AND value->>'registry_instance_id'<>'00000000-0000-0000-0000-000000000000'
    AND value->>'space_id'<>'00000000-0000-0000-0000-000000000000',false)
$$;
CREATE FUNCTION tracker_execution_start_space_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task uuid; project uuid; assignment uuid; scope jsonb; required jsonb; actor jsonb;
    deadline bigint; current_time_seconds bigint; entry jsonb; candidate record; origin text; assignment_revision bigint;
    enforced boolean; matched boolean:=false; required_matched boolean:=false; affected integer;
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
CREATE TRIGGER tracker_execution_start_space_guard BEFORE INSERT ON sdlc_assignments
    FOR EACH ROW EXECUTE FUNCTION tracker_execution_start_space_guard();
CREATE TRIGGER tracker_analysis_start_space_guard BEFORE INSERT ON sdlc_analysis_capacity_holds
    FOR EACH ROW EXECUTE FUNCTION tracker_execution_start_space_guard();

-- The canonical commit and pending delivery are durable together. ACK is separate
-- from both the user request lifetime and the latest project link revision.
CREATE TABLE tracker_project_space_outbox (
    operation_id uuid PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    revision bigint NOT NULL CHECK(revision>0),
    command jsonb NOT NULL,
    actor_identity jsonb NOT NULL,
    readback jsonb,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    confirmed_at timestamptz,
    UNIQUE(project_id,revision),
    CHECK((readback IS NULL)=(confirmed_at IS NULL))
);
CREATE TABLE tracker_project_space_operations (
    operation_id uuid PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    actor_identity jsonb NOT NULL,
    requested_change jsonb NOT NULL,
    projection_operation_id uuid NOT NULL REFERENCES tracker_project_space_outbox(operation_id) ON DELETE RESTRICT,
    authority_valid_until bigint NOT NULL CHECK(authority_valid_until>0),
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Durable original request metadata; replay returns current state, never restores
-- an old desired state. An expiring permit does not expire this audit receipt.
CREATE TABLE tracker_project_lifecycle_operations (
 operation_id uuid PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
 actor_identity jsonb NOT NULL,command jsonb NOT NULL,
 authority_valid_until bigint NOT NULL CHECK(authority_valid_until>0),created_at timestamptz NOT NULL DEFAULT now(),
 CHECK(coalesce(jsonb_typeof(command)='object'
  AND command-ARRAY['operation_id','expected_revision','archived']='{}'::jsonb
  AND command ?& ARRAY['operation_id','expected_revision','archived']
  AND command->>'operation_id'=operation_id::text
  AND jsonb_typeof(command->'expected_revision')='number'
  AND (command->>'expected_revision')::bigint>0
  AND jsonb_typeof(command->'archived')='boolean',false)),
 CHECK(coalesce(jsonb_typeof(actor_identity)='object'
  AND actor_identity-ARRAY['issuer','subject']='{}'::jsonb
  AND actor_identity ?& ARRAY['issuer','subject']
  AND jsonb_typeof(actor_identity->'issuer')='string'
  AND jsonb_typeof(actor_identity->'subject')='string'
  AND NULLIF(btrim(actor_identity->>'issuer'),'') IS NOT NULL
  AND NULLIF(btrim(actor_identity->>'subject'),'') IS NOT NULL,false))
);
CREATE FUNCTION tracker_project_lifecycle_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.authority_valid_until<=floor(extract(epoch FROM clock_timestamp()))::bigint THEN
  RAISE EXCEPTION 'project_lifecycle_authority_expired' USING ERRCODE='42501';
 END IF;RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER tracker_project_lifecycle_commit_guard AFTER INSERT ON tracker_project_lifecycle_operations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_project_lifecycle_commit_guard();

-- Operation-local exception for exactly the two lifecycle columns. It never
-- permits name/owner/content edits on an archived project or renews the proof.
CREATE FUNCTION tracker_project_lifecycle_transition_allowed(previous jsonb,desired jsonb) RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT coalesce(previous-ARRAY['archived_at','project_lifecycle_revision']=desired-ARRAY['archived_at','project_lifecycle_revision']
 AND (desired->>'project_lifecycle_revision')::bigint=(previous->>'project_lifecycle_revision')::bigint+1
 AND (previous->>'archived_at' IS NULL)<>(desired->>'archived_at' IS NULL)
 AND EXISTS(SELECT 1 FROM tracker_project_lifecycle_operations op
  WHERE op.operation_id::text=NULLIF(current_setting('pdlc.tracker.project_lifecycle_operation',true),'')
  AND op.project_id=(previous->>'id')::uuid
  AND op.authority_valid_until>floor(extract(epoch FROM clock_timestamp()))::bigint
  AND (op.command->>'expected_revision')::bigint=(previous->>'project_lifecycle_revision')::bigint
  AND (op.command->>'archived')::boolean=(desired->>'archived_at' IS NOT NULL)),false)
$$;
-- A permanent-delete command owns its children; it is not an impersonated
-- personal unsubscribe and may remove incoming links without exposing their data.
-- The scope is transaction-local, rechecks the trashed parent and original actor,
-- and never authorizes INSERT/UPDATE, an unrelated issue, or a new job.
CREATE FUNCTION tracker_is_issue_purge_child(table_name text,operation text,previous jsonb) RETURNS boolean LANGUAGE plpgsql VOLATILE AS $$
DECLARE scope jsonb; actor jsonb; parent_row jsonb;
BEGIN
 IF operation<>'DELETE' OR table_name<>ALL(ARRAY['comments','worklogs','attachments','issue_links','issue_labels','issue_watchers','issue_votes','issue_custom_field_values','issue_status_history','task_repository_links']) THEN RETURN false;END IF;
 BEGIN scope:=NULLIF(current_setting('pdlc.tracker.issue_purge',true),'')::jsonb;EXCEPTION WHEN invalid_text_representation THEN RAISE EXCEPTION 'invalid_issue_purge_scope' USING ERRCODE='42501';END;
 IF scope IS NULL THEN RETURN false;END IF;
 IF table_name='issue_links' THEN
  IF NOT coalesce(previous->>'source_id'=scope->>'issue_id' OR previous->>'target_id'=scope->>'issue_id',false) THEN RETURN false;END IF;
 ELSIF previous->>'issue_id' IS DISTINCT FROM scope->>'issue_id' THEN RETURN false;END IF;
 actor:=tracker_current_write_actor();
 IF actor->'identity' IS DISTINCT FROM scope->'identity' OR actor->>'user_id' IS DISTINCT FROM scope->>'user_id'
  OR (scope->>'valid_until')::bigint IS DISTINCT FROM (actor->>'valid_until')::bigint THEN RAISE EXCEPTION 'issue_purge_actor_changed' USING ERRCODE='42501';END IF;
 SELECT to_jsonb(i) INTO parent_row FROM issues i WHERE i.id=(scope->>'issue_id')::uuid FOR SHARE;
 IF NOT FOUND OR parent_row->>'project_id' IS DISTINCT FROM scope->>'project_id' OR parent_row->>'deleted_at' IS NULL THEN RAISE EXCEPTION 'issue_purge_parent_changed' USING ERRCODE='42501';END IF;
 PERFORM tracker_authorize_project_content('issues','DELETE',parent_row,NULL);
 RETURN true;
END $$;

CREATE FUNCTION tracker_project_archive_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE data jsonb; project uuid; ids uuid[]:='{}'; state record;
BEGIN
 IF TG_OP='DELETE' AND tracker_is_issue_purge_child(TG_TABLE_NAME,TG_OP,to_jsonb(OLD)) THEN RETURN OLD;END IF;
 IF TG_TABLE_NAME='projects' THEN
  IF TG_OP='INSERT' THEN
   IF NEW.archived_at IS NOT NULL OR NEW.project_lifecycle_revision<>1 THEN RAISE EXCEPTION 'project_lifecycle_command_required' USING ERRCODE='42501';END IF;RETURN NEW;
  ELSIF TG_OP='UPDATE' THEN
   IF OLD.archived_at IS DISTINCT FROM NEW.archived_at OR OLD.project_lifecycle_revision IS DISTINCT FROM NEW.project_lifecycle_revision THEN
    IF NOT tracker_project_lifecycle_transition_allowed(to_jsonb(OLD),to_jsonb(NEW)) THEN RAISE EXCEPTION 'project_lifecycle_command_required' USING ERRCODE='42501';END IF;RETURN NEW;
   END IF;
  ELSIF EXISTS(SELECT 1 FROM tracker_project_spaces WHERE project_id=OLD.id) THEN
   RAISE EXCEPTION 'linked_project_requires_archive' USING ERRCODE='42501';
  END IF;
 END IF;
 FOR data IN SELECT value FROM jsonb_array_elements(CASE TG_OP WHEN 'INSERT' THEN jsonb_build_array(to_jsonb(NEW)) WHEN 'DELETE' THEN jsonb_build_array(to_jsonb(OLD)) ELSE jsonb_build_array(to_jsonb(OLD),to_jsonb(NEW)) END) LOOP
  IF TG_TABLE_NAME='projects' THEN ids:=array_append(ids,(data->>'id')::uuid);
  ELSIF data ? 'project_id' THEN ids:=array_append(ids,(data->>'project_id')::uuid);
  ELSE
   FOR project IN SELECT project_id FROM issues WHERE id IN ((data->>'issue_id')::uuid,(data->>'source_id')::uuid) OR (TG_TABLE_NAME<>'issue_links' AND id=(data->>'target_id')::uuid) LOOP ids:=array_append(ids,project);END LOOP;
  END IF;
 END LOOP;
 -- Project-first shared locks serialize every protected content write with archive.
 FOR state IN SELECT id,archived_at FROM projects WHERE id=ANY(ids) ORDER BY id FOR SHARE LOOP
  IF state.archived_at IS NOT NULL THEN RAISE EXCEPTION 'project_archived_read_only' USING ERRCODE='42501';END IF;
 END LOOP;
 IF TG_OP='DELETE' THEN RETURN OLD;END IF;RETURN NEW;
END $$;
-- Only project content/personal project actions previously covered by the
-- legacy read-only rule. Control/terminal receipts, durable delivery/audit and
-- notification read-state are deliberately separate from content mutations.
DO $$ DECLARE name text;BEGIN
 FOREACH name IN ARRAY ARRAY['projects','issues','boards','sprints','project_members','labels','project_components','project_versions','comments','worklogs','attachments','issue_links','issue_labels','issue_watchers','issue_votes','custom_fields','issue_custom_field_values','issue_status_history','project_issue_counters','issue_creation_receipts','task_repository_links','project_repository_links'] LOOP
  IF to_regclass(format('%I.%I',current_schema(),name)) IS NOT NULL THEN EXECUTE format('CREATE TRIGGER project_archive_write_guard BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION tracker_project_archive_write_guard()',name);END IF;
 END LOOP;
END $$;


-- Product-owned common write policy. Context is minted by the verified server
-- boundary, transaction-local and contains no token. No SQL text classification.
CREATE FUNCTION tracker_current_write_actor() RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE actor jsonb; deadline bigint; entry jsonb; now_seconds bigint;
BEGIN
 BEGIN actor:=NULLIF(current_setting('pdlc.tracker.write_actor',true),'')::jsonb;deadline:=(actor->>'valid_until')::bigint;
 EXCEPTION WHEN invalid_text_representation OR numeric_value_out_of_range THEN RAISE EXCEPTION 'invalid_project_write_context' USING ERRCODE='42501';END;
 now_seconds:=floor(extract(epoch FROM clock_timestamp()))::bigint;
 IF actor IS NULL OR jsonb_typeof(actor) IS DISTINCT FROM 'object'
  OR actor->>'schema_version' IS DISTINCT FROM '1' OR coalesce(actor->>'kind','') NOT IN ('human','create_project')
  OR jsonb_typeof(actor->'user_id') IS DISTINCT FROM 'string'
  OR jsonb_typeof(actor->'policy_revision') IS DISTINCT FROM 'number' OR NOT coalesce(actor->>'policy_revision' ~ '^[1-9][0-9]*$',false)
  OR actor->'writable' IS DISTINCT FROM 'true'::jsonb OR deadline IS NULL OR deadline<=now_seconds OR deadline>now_seconds+60
  OR jsonb_typeof(actor->'identity') IS DISTINCT FROM 'object'
  OR (actor->'identity')-ARRAY['issuer','subject']<>'{}'::jsonb OR NOT ((actor->'identity') ?& ARRAY['issuer','subject'])
  OR jsonb_typeof(actor->'identity'->'issuer') IS DISTINCT FROM 'string' OR jsonb_typeof(actor->'identity'->'subject') IS DISTINCT FROM 'string'
  OR NULLIF(btrim(actor->'identity'->>'issuer'),'') IS NULL OR NULLIF(btrim(actor->'identity'->>'subject'),'') IS NULL
  OR jsonb_typeof(actor->'active_spaces') IS DISTINCT FROM 'array' OR jsonb_typeof(actor->'readable_spaces') IS DISTINCT FROM 'array' THEN RAISE EXCEPTION 'project_write_context_required' USING ERRCODE='42501';END IF;
 IF jsonb_array_length(actor->'active_spaces')>10000 OR jsonb_array_length(actor->'readable_spaces')>10000 THEN RAISE EXCEPTION 'invalid_project_write_context' USING ERRCODE='42501';END IF;
 FOR entry IN SELECT value FROM jsonb_array_elements((actor->'active_spaces')||(actor->'readable_spaces')) LOOP
  IF NOT tracker_valid_space_ref(entry) THEN RAISE EXCEPTION 'invalid_project_write_space' USING ERRCODE='42501';END IF;
 END LOOP;
 BEGIN IF (actor->>'user_id')::uuid IS NULL OR (actor->>'user_id')::uuid='00000000-0000-0000-0000-000000000000' THEN RAISE EXCEPTION 'invalid_project_write_user' USING ERRCODE='42501';END IF;
 EXCEPTION WHEN invalid_text_representation THEN RAISE EXCEPTION 'invalid_project_write_user' USING ERRCODE='42501';END;
 RETURN actor;
END $$;
CREATE FUNCTION tracker_lock_write_user(actor jsonb) RETURNS uuid LANGUAGE plpgsql VOLATILE AS $$
DECLARE profile record; subject uuid;
BEGIN
 subject:=(actor->>'user_id')::uuid;
 SELECT id,is_active,central_sub INTO profile FROM users WHERE id=subject FOR SHARE;
 IF NOT FOUND OR NOT profile.is_active OR profile.central_sub IS DISTINCT FROM actor->'identity'->>'subject' THEN RAISE EXCEPTION 'project_write_user_not_confirmed' USING ERRCODE='42501';END IF;
 RETURN subject;
END $$;
CREATE FUNCTION tracker_authorize_project_content(table_name text,operation text,previous jsonb,desired jsonb) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE actor jsonb; subject uuid; data jsonb; project uuid; project_ids uuid[]:='{}'; edit_projects uuid[]:='{}';
 issue uuid; issue_ids uuid[]:='{}'; mapping jsonb:='[]'; item jsonb; actual uuid; parent record; candidate record; allowed boolean;
 own_action boolean; owner_required boolean; creating boolean;
BEGIN
 IF table_name<>ALL(ARRAY['projects','issues','boards','sprints','project_members','labels','project_components','project_versions','comments','worklogs','attachments','issue_links','issue_labels','issue_watchers','issue_votes','custom_fields','issue_custom_field_values','issue_status_history','project_issue_counters','issue_creation_receipts','task_repository_links','project_repository_links']) THEN RAISE EXCEPTION 'scoped_machine_context_required' USING ERRCODE='42501';END IF;
 actor:=tracker_current_write_actor();
 IF tracker_is_issue_purge_child(table_name,operation,previous) THEN RETURN;END IF;
 creating:=table_name='projects' AND operation='INSERT';
 IF creating THEN
  IF actor->>'kind'<>'create_project' OR actor->>'project_id' IS DISTINCT FROM desired->>'id' OR actor->>'user_id' IS DISTINCT FROM desired->>'owner_id' THEN RAISE EXCEPTION 'project_creation_context_required' USING ERRCODE='42501';END IF;
  subject:=tracker_lock_write_user(actor);
  IF jsonb_typeof(actor->'requested_spaces') IS DISTINCT FROM 'array' THEN RAISE EXCEPTION 'project_spaces_required' USING ERRCODE='42501';END IF;
  IF jsonb_array_length(actor->'requested_spaces') NOT BETWEEN 1 AND 1000 THEN RAISE EXCEPTION 'project_spaces_required' USING ERRCODE='42501';END IF;
  FOR item IN SELECT value FROM jsonb_array_elements(actor->'requested_spaces') LOOP
   IF NOT tracker_valid_space_ref(item) OR NOT actor->'active_spaces' @> jsonb_build_array(item) THEN RAISE EXCEPTION 'project_creation_space_denied' USING ERRCODE='42501';END IF;
  END LOOP;
  FOR candidate IN SELECT c.* FROM tracker_space_catalog c WHERE actor->'requested_spaces' @> jsonb_build_array(jsonb_build_object('registry_instance_id',c.registry_instance_id,'space_id',c.space_id)) ORDER BY c.registry_instance_id,c.space_id FOR SHARE LOOP
   IF candidate.admin_revision IS NULL OR candidate.state<>'active' THEN RAISE EXCEPTION 'project_creation_space_not_active' USING ERRCODE='42501';END IF;
  END LOOP;
  IF (SELECT count(*) FROM tracker_space_catalog c WHERE actor->'requested_spaces' @> jsonb_build_array(jsonb_build_object('registry_instance_id',c.registry_instance_id,'space_id',c.space_id)))<>jsonb_array_length(actor->'requested_spaces') THEN RAISE EXCEPTION 'project_creation_space_not_confirmed' USING ERRCODE='42501';END IF;
  RETURN;
 END IF;
 -- Resolve the owning resource, then lock/recheck its mapping. A concurrent
 -- Issue move cannot turn an old MVCC project ID into authority for a new one.
 FOR data IN SELECT value FROM jsonb_array_elements(CASE operation WHEN 'INSERT' THEN jsonb_build_array(desired) WHEN 'DELETE' THEN jsonb_build_array(previous) ELSE jsonb_build_array(previous,desired) END) LOOP
  IF table_name='projects' THEN project_ids:=array_append(project_ids,(data->>'id')::uuid);
  ELSIF data ? 'project_id' THEN project_ids:=array_append(project_ids,(data->>'project_id')::uuid);
  ELSE
   FOR issue IN SELECT DISTINCT id FROM unnest(ARRAY[(data->>'issue_id')::uuid,(data->>'source_id')::uuid,CASE WHEN table_name='issue_links' AND operation='DELETE' THEN NULL ELSE (data->>'target_id')::uuid END]) id WHERE id IS NOT NULL ORDER BY id LOOP
    SELECT project_id INTO project FROM issues WHERE id=issue;
    IF NOT FOUND THEN
     -- Only an already authorized parent purge may explain a missing FK parent.
     BEGIN project:=(NULLIF(current_setting('pdlc.tracker.deleted_issues',true),'')::jsonb->>issue::text)::uuid;EXCEPTION WHEN invalid_text_representation THEN project:=NULL;END;
     IF operation<>'DELETE' OR project IS NULL THEN RAISE EXCEPTION 'project_resource_mapping_missing' USING ERRCODE='42501';END IF;
    ELSE mapping:=mapping||jsonb_build_array(jsonb_build_object('issue',issue,'project',project));END IF;
    IF table_name='issue_links' AND issue=(data->>'source_id')::uuid THEN edit_projects:=array_append(edit_projects,project);END IF;
    project_ids:=array_append(project_ids,project);issue_ids:=array_append(issue_ids,issue);
   END LOOP;
  END IF;
 END LOOP;
 IF array_length(project_ids,1) IS NULL OR array_position(project_ids,NULL) IS NOT NULL THEN RAISE EXCEPTION 'project_resource_mapping_missing' USING ERRCODE='42501';END IF;
 PERFORM id FROM projects WHERE id=ANY(project_ids) ORDER BY id FOR SHARE;
 FOR item IN SELECT value FROM jsonb_array_elements(mapping) ORDER BY value->>'issue' LOOP
  SELECT project_id INTO actual FROM issues WHERE id=(item->>'issue')::uuid FOR SHARE;
  IF NOT FOUND OR actual IS DISTINCT FROM (item->>'project')::uuid THEN RAISE EXCEPTION 'project_resource_mapping_changed' USING ERRCODE='40001';END IF;
 END LOOP;
 subject:=tracker_lock_write_user(actor);own_action:=table_name IN ('issue_watchers','issue_votes');owner_required:=table_name IN ('projects','project_members');
 IF own_action THEN
  FOR data IN SELECT value FROM jsonb_array_elements(CASE operation WHEN 'INSERT' THEN jsonb_build_array(desired) WHEN 'DELETE' THEN jsonb_build_array(previous) ELSE jsonb_build_array(previous,desired) END) LOOP
   IF data->>'user_id' IS DISTINCT FROM subject::text THEN RAISE EXCEPTION 'personal_project_action_denied' USING ERRCODE='42501';END IF;
  END LOOP;
  IF table_name='issue_votes' AND operation<>'DELETE' AND EXISTS(SELECT 1 FROM issues WHERE id=(desired->>'issue_id')::uuid AND reporter_id=subject) THEN RAISE EXCEPTION 'cannot_vote_for_own_issue' USING ERRCODE='42501';END IF;
 END IF;
 FOR project IN SELECT DISTINCT id FROM unnest(project_ids) id ORDER BY id LOOP
  IF actor->>'kind'='create_project' AND actor->>'project_id' IS DISTINCT FROM project::text THEN RAISE EXCEPTION 'project_creation_scope_mismatch' USING ERRCODE='42501';END IF;
  SELECT id,owner_id,archived_at INTO parent FROM projects WHERE id=project;
  IF NOT FOUND OR (parent.archived_at IS NOT NULL AND (table_name<>'issue_links' OR project=ANY(edit_projects))) THEN RAISE EXCEPTION 'project_archived_or_missing' USING ERRCODE='42501';END IF;
  IF parent.owner_id<>subject AND NOT own_action AND (table_name<>'issue_links' OR project=ANY(edit_projects)) THEN
   IF owner_required THEN RAISE EXCEPTION 'project_owner_required' USING ERRCODE='42501';END IF;
   PERFORM user_id FROM project_members WHERE project_id=project AND user_id=subject FOR SHARE;
   IF NOT FOUND THEN RAISE EXCEPTION 'project_edit_role_required' USING ERRCODE='42501';END IF;
  END IF;
  allowed:=false;
  FOR candidate IN SELECT c.* FROM tracker_project_spaces l JOIN tracker_space_catalog c USING(registry_instance_id,space_id) WHERE l.project_id=project ORDER BY c.registry_instance_id,c.space_id FOR SHARE OF c LOOP
   IF candidate.admin_revision IS NOT NULL AND (
    (table_name='issue_links' AND NOT project=ANY(edit_projects) AND candidate.state IN ('active','archived') AND actor->'readable_spaces' @> jsonb_build_array(jsonb_build_object('registry_instance_id',candidate.registry_instance_id,'space_id',candidate.space_id)))
    OR (candidate.state='active' AND actor->'active_spaces' @> jsonb_build_array(jsonb_build_object('registry_instance_id',candidate.registry_instance_id,'space_id',candidate.space_id)))) THEN allowed:=true;END IF;
  END LOOP;
  IF NOT allowed THEN RAISE EXCEPTION 'active_admitted_project_space_required' USING ERRCODE='42501';END IF;
 END LOOP;
 IF table_name='issues' AND operation='DELETE' THEN
  PERFORM set_config('pdlc.tracker.deleted_issues',(coalesce(NULLIF(current_setting('pdlc.tracker.deleted_issues',true),'')::jsonb,'{}'::jsonb)||jsonb_build_object(previous->>'id',previous->>'project_id'))::text,true);
 END IF;
END $$;

CREATE FUNCTION tracker_begin_issue_purge(issue uuid) RETURNS void LANGUAGE plpgsql VOLATILE AS $$
DECLARE original jsonb; locked jsonb; actor jsonb;
BEGIN
 PERFORM singleton FROM tracker_space_admission_state WHERE singleton FOR SHARE;
 IF NOT FOUND THEN RAISE EXCEPTION 'space_admission_state_missing' USING ERRCODE='55000';END IF;
 IF NOT ((SELECT active FROM tracker_space_admission_state WHERE singleton) OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL)) THEN RETURN;END IF;
 SELECT to_jsonb(i) INTO original FROM issues i WHERE i.id=issue;
 IF NOT FOUND OR original->>'deleted_at' IS NULL THEN RAISE EXCEPTION 'trashed_issue_required' USING ERRCODE='42501';END IF;
 -- Acquire the project's role/catalog barriers before the issue row lock.
 PERFORM tracker_authorize_project_content('issues','DELETE',original,NULL);
 SELECT to_jsonb(i) INTO locked FROM issues i WHERE i.id=issue FOR UPDATE;
 IF NOT FOUND OR locked->>'project_id' IS DISTINCT FROM original->>'project_id' OR locked->>'deleted_at' IS NULL THEN RAISE EXCEPTION 'issue_purge_parent_changed' USING ERRCODE='40001';END IF;
 actor:=tracker_current_write_actor();
 PERFORM set_config('pdlc.tracker.issue_purge',jsonb_build_object('issue_id',issue,'project_id',locked->>'project_id','user_id',actor->>'user_id','identity',actor->'identity','valid_until',actor->'valid_until')::text,true);
END $$;

-- Additive replacement; historical 0090 and its checksum remain unchanged.
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
        PERFORM tracker_authorize_project_content(TG_TABLE_NAME,TG_OP,to_jsonb(OLD),to_jsonb(NEW));
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

-- The original links request must remain valid through commit, including no-op.
-- This receipt is durable metadata, never a renewable authority for later replay.
CREATE FUNCTION tracker_project_spaces_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF NEW.authority_valid_until<=floor(extract(epoch FROM clock_timestamp()))::bigint THEN RAISE EXCEPTION 'project_space_authority_expired' USING ERRCODE='42501';END IF;RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER tracker_project_spaces_commit_guard AFTER INSERT ON tracker_project_space_operations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_project_spaces_commit_guard();

CREATE FUNCTION tracker_content_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF (SELECT active FROM tracker_space_admission_state WHERE singleton) OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) THEN
  IF TG_TABLE_NAME='projects' AND TG_OP='UPDATE' AND tracker_project_lifecycle_transition_allowed(to_jsonb(OLD),to_jsonb(NEW)) THEN RETURN NULL;END IF;
  PERFORM tracker_current_write_actor();
 END IF;RETURN NULL;
END $$;
DO $$ DECLARE name text;BEGIN
 FOREACH name IN ARRAY ARRAY['projects','issues','boards','sprints','project_members','labels','project_components','project_versions','comments','worklogs','attachments','issue_links','issue_labels','issue_watchers','issue_votes','custom_fields','issue_custom_field_values','issue_status_history','project_issue_counters','issue_creation_receipts','task_repository_links','project_repository_links'] LOOP
  IF to_regclass(format('%I.%I',current_schema(),name)) IS NOT NULL THEN
   -- A few newer tables did not have a legacy namespace trigger.
   IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid=to_regclass(format('%I.%I',current_schema(),name)) AND tgname='namespace_write_guard') THEN EXECUTE format('CREATE TRIGGER namespace_write_guard BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard()',name);END IF;
   EXECUTE format('CREATE CONSTRAINT TRIGGER shared_content_commit_guard AFTER INSERT OR UPDATE OR DELETE ON %I DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_content_commit_guard()',name);
  END IF;
 END LOOP;
END $$;
