ALTER TABLE projects ADD COLUMN namespace_managed boolean NOT NULL DEFAULT false;
CREATE TABLE project_issue_counters (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    high_water_mark bigint NOT NULL CHECK (high_water_mark BETWEEN 0 AND 4294967295)
);
-- Includes trashed issues. Numbers already purged before migration cannot be inferred.
INSERT INTO project_issue_counters(project_id,high_water_mark)
SELECT p.id,COALESCE(MAX((substring(i.key FROM '-([0-9]+)$'))::bigint),0)
FROM projects p LEFT JOIN issues i ON i.project_id=p.id GROUP BY p.id;
CREATE FUNCTION allocate_project_issue_number(project uuid) RETURNS bigint LANGUAGE sql AS $$
    INSERT INTO project_issue_counters(project_id,high_water_mark) VALUES(project,1)
    ON CONFLICT(project_id) DO UPDATE SET high_water_mark=project_issue_counters.high_water_mark+1
    RETURNING high_water_mark;
$$;
CREATE TABLE tracker_namespace_bindings (
    resource_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE RESTRICT,
    registry_instance_id uuid NOT NULL,
    namespace_id uuid NOT NULL,
    generation bigint NOT NULL CHECK(generation > 0),
    state text NOT NULL CHECK(state IN ('active','archived')),
    command jsonb NOT NULL,
    UNIQUE(registry_instance_id,namespace_id)
);
-- This lock closes the race between an admitted write and lifecycle transition.
CREATE FUNCTION tracker_namespace_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE data jsonb; pid uuid; pids uuid[] := '{}'; lifecycle text;
BEGIN
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


ALTER TABLE tracker_namespace_bindings ADD CONSTRAINT namespace_projection_consistent CHECK (COALESCE((
    command->>'schema_version'='1' AND command->'namespace'->>'registry_instance_id'=registry_instance_id::text
    AND command->'namespace'->>'namespace_id'=namespace_id::text AND command->'resource'->>'resource_id'=resource_id::text
    AND command->'resource'->>'kind'='tracker_project' AND command->>'generation'=generation::text AND command->>'state'=state
    AND command ? 'operation_id' AND command->'resource' ? 'instance_id'
    AND jsonb_typeof(command->'namespace')='object' AND jsonb_typeof(command->'resource')='object'
),false)
);

-- Receipts survive issue purge; the allocated number and UUID never change.
CREATE TABLE task_repository_links (
    issue_id uuid NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    forge_instance_id uuid NOT NULL, repository_id uuid NOT NULL, snapshot jsonb NOT NULL,
    created_by uuid NOT NULL REFERENCES users(id), created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(issue_id,forge_instance_id,repository_id)
);
CREATE FUNCTION tracker_mark_namespace_resource() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 UPDATE projects SET namespace_managed=true WHERE id=NEW.resource_id AND NOT namespace_managed;
 RETURN NEW;
END $$;
CREATE TRIGGER tracker_namespace_resource_marker AFTER INSERT ON tracker_namespace_bindings FOR EACH ROW EXECUTE FUNCTION tracker_mark_namespace_resource();
CREATE TABLE issue_creation_receipts (
    operation_id uuid PRIMARY KEY, project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    actor_id uuid NOT NULL, payload jsonb NOT NULL, issue_id uuid NOT NULL UNIQUE,
    number bigint NOT NULL CHECK(number BETWEEN 1 AND 4294967295), completed boolean NOT NULL DEFAULT false
);

DO $$ DECLARE table_name text; BEGIN
    FOREACH table_name IN ARRAY ARRAY['projects','issues','boards','sprints','project_members','labels',
        'project_components','project_versions','comments','worklogs','attachments','issue_links',
        'issue_labels','issue_watchers','issue_votes','custom_fields','issue_custom_field_values','project_issue_counters',
        'issue_creation_receipts','task_repository_links','sdlc_tasks','sdlc_draft_creations','sdlc_agent_bindings','sdlc_assignments',
        'sdlc_pm_executions','sdlc_pm_execution_leases','sdlc_pm_lease_operations','sdlc_requests',
        'sdlc_question_versions','sdlc_options','sdlc_requirements','sdlc_answers','sdlc_evidence','sdlc_confirmations',
        'sdlc_project_routing_revisions','sdlc_project_routing_heads','sdlc_project_routing_operations',
        'sdlc_task_routing_snapshots','sdlc_analysis_intents','sdlc_analysis_reservations',
        'sdlc_analysis_reservation_leases','sdlc_analysis_reservation_operations','sdlc_idempotency','sdlc_outbox']
    LOOP
        EXECUTE format('CREATE TRIGGER namespace_write_guard BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard()',table_name);
    END LOOP;
END $$;

-- Legacy writes may not rely on an unreadable strict-v1 owner projection.
ALTER TABLE tracker_namespace_bindings ADD CONSTRAINT namespace_projection_shape CHECK (COALESCE((
    command - ARRAY['schema_version','namespace','resource','operation_id','generation','state','create_spec'] = '{}'::jsonb
    AND command ?& ARRAY['schema_version','namespace','resource','operation_id','generation','state']
    AND jsonb_typeof(command->'schema_version')='number' AND jsonb_typeof(command->'generation')='number'
    AND (command->'namespace') - ARRAY['registry_instance_id','namespace_id'] = '{}'::jsonb
    AND (command->'resource') - ARRAY['kind','instance_id','resource_id'] = '{}'::jsonb
    AND command->>'operation_id' ~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
    AND command->'resource'->>'instance_id' ~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
    AND command->>'operation_id'<>'00000000-0000-0000-0000-000000000000'
    AND command->'resource'->>'instance_id'<>'00000000-0000-0000-0000-000000000000'
    AND registry_instance_id<>'00000000-0000-0000-0000-000000000000'::uuid
    AND namespace_id<>'00000000-0000-0000-0000-000000000000'::uuid
    AND resource_id<>'00000000-0000-0000-0000-000000000000'::uuid
    AND (NOT command ? 'create_spec' OR command->'create_spec'='null'::jsonb OR jsonb_typeof(command->'create_spec')='object')
),false));
