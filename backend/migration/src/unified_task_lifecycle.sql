-- Schema first. Activation/import is an explicit, separately qualified operation.
ALTER TABLE sdlc_tasks ADD COLUMN core_revision bigint NOT NULL DEFAULT 1
    CHECK(core_revision BETWEEN 1 AND 9007199254740991);
-- Execution ownership is separate from the business stage. Existing receipts stay immutable.
ALTER TABLE sdlc_tasks ADD COLUMN manual_control boolean NOT NULL DEFAULT false;
CREATE TABLE task_lifecycle_custody_history (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    core_revision bigint NOT NULL,
    previous_state jsonb NOT NULL,
    previous_execution jsonb,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(task_id,core_revision)
);
CREATE TRIGGER task_lifecycle_custody_immutable BEFORE UPDATE OR DELETE ON task_lifecycle_custody_history
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE TABLE task_lifecycle_control (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    enabled boolean NOT NULL DEFAULT false
);
INSERT INTO task_lifecycle_control(singleton) VALUES(true);

CREATE TABLE task_lifecycle_history (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    actor_id uuid NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    action text NOT NULL CHECK(action IN ('created','imported','start','complete','request_rework','reopen')),
    previous_state jsonb,
    next_state jsonb NOT NULL,
    next_submode text CHECK(next_submode IN ('brainstorming','clarification','awaiting_confirmation')),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    command_hash text NOT NULL CHECK(command_hash ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(task_id,actor_id,idempotency_key)
);
CREATE TRIGGER task_lifecycle_history_immutable BEFORE UPDATE OR DELETE ON task_lifecycle_history
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
-- Durable invalidation, committed with the state. Dispatch is repeatable; clients refetch.
CREATE TABLE task_lifecycle_events (
    sequence bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    revision bigint NOT NULL,
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(task_id,revision)
);
CREATE FUNCTION task_lifecycle_emit_event() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO task_lifecycle_events(task_id,revision,project_id)
        VALUES(NEW.task_id,NEW.core_revision,NEW.project_id);
    PERFORM pg_notify('pdlc_task_lifecycle',jsonb_build_object('type','issue_updated',
        'issue_id',NEW.task_id::text,'project_key',(SELECT key FROM projects WHERE id=NEW.project_id))::text);
    RETURN NULL;
END $$;
CREATE TRIGGER task_lifecycle_created_event AFTER INSERT ON sdlc_tasks
FOR EACH ROW EXECUTE FUNCTION task_lifecycle_emit_event();
CREATE TRIGGER task_lifecycle_updated_event AFTER UPDATE ON sdlc_tasks
FOR EACH ROW WHEN (NEW.state IS DISTINCT FROM OLD.state)
EXECUTE FUNCTION task_lifecycle_emit_event();

CREATE TABLE task_lifecycle_write_permits (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    transaction_id xid8 NOT NULL,
    next_state jsonb NOT NULL,
    PRIMARY KEY(task_id,transaction_id)
);

-- Shared read projection, retaining old state and receipts byte-for-byte.
CREATE FUNCTION task_lifecycle_effective_stage(snapshot jsonb) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN snapshot->>'stage'='Clarification' THEN 'Draft' ELSE snapshot->>'stage' END
$$;
CREATE FUNCTION task_lifecycle_effective_status(snapshot jsonb) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN snapshot->>'stage' IN ('Draft','Clarification')
        AND snapshot->>'status' IS NULL AND NOT snapshot ? 'flow' THEN 'Ready'
        ELSE snapshot->>'status' END
$$;

CREATE FUNCTION task_lifecycle_status_id(stage text, status text) RETURNS uuid
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT md5('pdlc.task-lifecycle.v1/'||stage||'/'||status)::uuid
$$;
-- Saved legacy reads retain exactly the four agreed import pairs; no writer authority.
CREATE FUNCTION task_lifecycle_legacy_status_id(projected uuid) RETURNS uuid
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT legacy_id::uuid FROM (VALUES
        ('00000000-0000-0000-0000-000000000001','Backlog','Ready'),
        ('00000000-0000-0000-0000-000000000002','Development','InProgress'),
        ('00000000-0000-0000-0000-000000000004','Review','Ready'),
        ('00000000-0000-0000-0000-000000000003','Deployment','Completed')
    ) legacy(legacy_id,stage,status)
    WHERE task_lifecycle_status_id(stage,status)=projected
$$;
INSERT INTO statuses(id,name,category,position,is_default,is_closed)
SELECT task_lifecycle_status_id(stage,status), 'Task '||stage||' / '||status,
    CASE WHEN stage='Deployment' AND status='Completed' THEN 'done'
         WHEN stage IN ('Draft','Backlog') AND status='Ready' THEN 'todo'
         ELSE 'inprogress' END,
    ordinal,false,stage='Deployment' AND status='Completed'
FROM (VALUES('Draft',0),('Backlog',1),('Analysis',2),('Architecture',3),
    ('Development',4),('Review',5),('Testing',6),('Rework',7),('Deployment',8)) stages(stage,ordinal)
CROSS JOIN (VALUES('Ready'),('InProgress'),('Completed')) phases(status);

-- A lease timeout is never evidence of completion. Match the current immutable receipt.
CREATE FUNCTION task_lifecycle_pm_quiescent(t sdlc_tasks) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT CASE WHEN t.pm_execution_id IS NULL THEN
        t.state->'assignment' IS NULL OR t.state->'assignment'='null'::jsonb
    ELSE t.pm_admission_state='completed' AND (
        EXISTS(SELECT 1 FROM sdlc_pm_completions c WHERE c.task_id=t.task_id
            AND c.execution_id=t.pm_execution_id
            AND c.assignment_id::text=t.state->'assignment'->>'assignment_id'
            AND c.assignment_version::text=t.state->'assignment'->>'version'
            AND c.result->>'next_pm_admission_state'='completed')
        )
        -- Configured PM completion proves Workflow publication, not native process
        -- quiescence. Its owner drain/continuation barrier is required separately;
        -- never manufacture a native completion just to satisfy this predicate.
    END IS TRUE
$$;

CREATE FUNCTION task_lifecycle_execution_busy(task uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=task)
        OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=task)
        OR EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads h WHERE h.task_id=task
            AND COALESCE(sdlc_critical_pm_detail(h.blocker_id)->>'state','unknown')<>'resolved')
        OR EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=task AND NOT t.manual_control AND (
            NOT task_lifecycle_pm_quiescent(t)
            OR t.state ? 'lifecycle' AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_heads h WHERE h.task_id=t.task_id)
            OR EXISTS(SELECT 1 FROM sdlc_delivery_heads h WHERE h.task_id=t.task_id AND (
                COALESCE(t.state->'lifecycle'->>'runtime','unknown') NOT IN ('queued','terminal')
                OR COALESCE(h.execution->'assignment','null'::jsonb)<>'null'::jsonb AND (
                    t.state->'lifecycle'->>'runtime' IS DISTINCT FROM 'terminal'
                    OR NOT (EXISTS(SELECT 1 FROM sdlc_delivery_results result
                        WHERE result.task_id=t.task_id
                        AND result.assignment_id::text=h.execution->'assignment'->>'assignment_id'
                        AND result.payload=h.execution->'result')
                    OR EXISTS(SELECT 1 FROM sdlc_delivery_reconciliations reconcile
                        WHERE reconcile.task_id=t.task_id
                        AND reconcile.assignment_id::text=h.execution->'assignment'->>'assignment_id'
                        AND reconcile.payload->'fence'->'execution_id'=h.execution->'assignment'->'execution_id'
                        AND reconcile.payload->'fence'->'fencing_token'=h.execution->'assignment'->'fencing_token')))))))
$$;

CREATE FUNCTION task_lifecycle_manual_task(task uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_tasks WHERE task_id=task AND manual_control)
$$;

CREATE FUNCTION task_lifecycle_projected_task(task uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=task
        AND (t.manual_control OR (SELECT enabled FROM task_lifecycle_control WHERE singleton)))
$$;

-- This exact permit applies only to a proven idle task under its locked owner row.
CREATE FUNCTION task_lifecycle_manual_write_permitted(task uuid, next_state jsonb) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT (SELECT enabled FROM task_lifecycle_control WHERE singleton)
        AND NOT task_lifecycle_execution_busy(task)
        AND EXISTS(SELECT 1 FROM task_lifecycle_write_permits p
            WHERE p.task_id=task AND p.transaction_id=pg_current_xact_id() AND p.next_state=$2)
$$;

CREATE FUNCTION task_lifecycle_state_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.task_id IS DISTINCT FROM OLD.task_id OR NEW.project_id IS DISTINCT FROM OLD.project_id
        OR NEW.owner_subject IS DISTINCT FROM OLD.owner_subject
        OR NEW.tracker_instance_id IS DISTINCT FROM OLD.tracker_instance_id
        OR NEW.root_task_id IS DISTINCT FROM OLD.root_task_id THEN
        RAISE EXCEPTION 'task_lifecycle_identity_immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.manual_control IS DISTINCT FROM OLD.manual_control
        OR OLD.manual_control AND NEW.state IS DISTINCT FROM OLD.state
        OR task_lifecycle_manual_write_permitted(OLD.task_id,NEW.state) THEN
        IF NOT NEW.manual_control
            OR NOT task_lifecycle_manual_write_permitted(OLD.task_id,NEW.state)
            OR (NEW.state-ARRAY['stage','status','lifecycle','runtime_state','waiting_reason','draft_submode'])
                IS DISTINCT FROM (OLD.state-ARRAY['stage','status','lifecycle','runtime_state','waiting_reason','draft_submode'])
            OR NEW.state ? 'lifecycle' OR NEW.state ? 'runtime_state' OR NEW.state ? 'waiting_reason'
            OR NEW.state ? 'draft_submode' AND NEW.state->>'stage'<>'Draft'
            OR NEW.pm_execution_id IS DISTINCT FROM OLD.pm_execution_id
            OR NEW.pm_owner_version IS DISTINCT FROM OLD.pm_owner_version
            OR NEW.pm_admission_state IS DISTINCT FROM OLD.pm_admission_state THEN
            RAISE EXCEPTION 'task_lifecycle_exact_manual_transition_required' USING ERRCODE='23514';
        END IF;
        IF NOT OLD.manual_control THEN
            INSERT INTO task_lifecycle_custody_history(task_id,core_revision,previous_state,previous_execution)
            VALUES(OLD.task_id,OLD.core_revision,OLD.state,
                (SELECT execution FROM sdlc_delivery_heads WHERE task_id=OLD.task_id));
        END IF;
    ELSIF OLD.manual_control AND (
        NEW.pm_execution_id IS DISTINCT FROM OLD.pm_execution_id
        OR NEW.pm_owner_version IS DISTINCT FROM OLD.pm_owner_version
        OR NEW.pm_admission_state IS DISTINCT FROM OLD.pm_admission_state) THEN
        RAISE EXCEPTION 'task_lifecycle_handoff_required' USING ERRCODE='23514';
    END IF;
    IF NEW.state IS DISTINCT FROM OLD.state THEN
        NEW.core_revision := OLD.core_revision+1;
    ELSIF NEW.core_revision IS DISTINCT FROM OLD.core_revision THEN
        RAISE EXCEPTION 'task_lifecycle_revision_immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER task_lifecycle_state_guard BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW EXECUTE FUNCTION task_lifecycle_state_guard();

-- The old Analysis trigger assumes every Analysis task has an accepted agent result.
-- Core-only tasks instead use the exact manual permit guard; execution aggregates
-- keep the unchanged historical function and all its result/fencing checks.
DROP TRIGGER sdlc_analysis_gate ON sdlc_tasks;
CREATE TRIGGER sdlc_analysis_gate BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW WHEN (NOT task_lifecycle_manual_write_permitted(OLD.task_id,NEW.state))
EXECUTE FUNCTION sdlc_analysis_gate();

-- Keep the execution guard unchanged for every historical/execution aggregate.
DROP TRIGGER sdlc_issue_gate ON issues;
CREATE TRIGGER sdlc_issue_gate BEFORE UPDATE ON issues
FOR EACH ROW WHEN (NOT task_lifecycle_projected_task(NEW.id)) EXECUTE FUNCTION sdlc_issue_gate();
CREATE FUNCTION task_lifecycle_issue_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task sdlc_tasks;
BEGIN
    SELECT * INTO task FROM sdlc_tasks WHERE task_id=NEW.id;
    IF NOT FOUND OR NOT task_lifecycle_projected_task(NEW.id) THEN RETURN NEW; END IF;
    IF NEW.project_id IS DISTINCT FROM task.project_id
        OR NEW.status_id IS DISTINCT FROM task_lifecycle_status_id(task_lifecycle_effective_stage(task.state),task_lifecycle_effective_status(task.state)) THEN
        RAISE EXCEPTION 'task_lifecycle_transition_required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER task_lifecycle_issue_guard BEFORE UPDATE ON issues
FOR EACH ROW EXECUTE FUNCTION task_lifecycle_issue_guard();

CREATE FUNCTION task_lifecycle_initialize_issue(task uuid, importing boolean DEFAULT false, importing_actor uuid DEFAULT NULL) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE i issues; owner text; instance text; stage text; status text; snapshot jsonb; actor uuid;
BEGIN
    IF NOT importing AND NOT (SELECT enabled FROM task_lifecycle_control WHERE singleton) THEN RETURN; END IF;
    SELECT * INTO STRICT i FROM issues WHERE id=task FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_tasks WHERE task_id=task) THEN RETURN; END IF;
    SELECT instance_id INTO STRICT instance FROM sdlc_instance WHERE singleton;
    SELECT COALESCE(central_sub,id::text) INTO STRICT owner FROM users WHERE id=i.reporter_id;
    actor:=CASE WHEN importing THEN importing_actor ELSE COALESCE(nullif(current_setting('pdlc.tracker.write_actor',true),'')::jsonb->>'user_id',i.reporter_id::text)::uuid END;
    IF importing AND actor IS NULL THEN RAISE EXCEPTION 'task_lifecycle_import_actor_required' USING ERRCODE='23514'; END IF;
    IF importing THEN
        CASE i.status_id
            WHEN '00000000-0000-0000-0000-000000000001'::uuid THEN stage:='Backlog';status:='Ready';
            WHEN '00000000-0000-0000-0000-000000000002'::uuid THEN stage:='Development';status:='InProgress';
            WHEN '00000000-0000-0000-0000-000000000004'::uuid THEN stage:='Review';status:='Ready';
            WHEN '00000000-0000-0000-0000-000000000003'::uuid THEN stage:='Deployment';status:='Completed';
            ELSE RAISE EXCEPTION 'task_lifecycle_unknown_legacy_status:%:%',task,i.status_id USING ERRCODE='23514';
        END CASE;
    ELSE stage:='Draft';status:='Ready'; END IF;
    snapshot:=jsonb_build_object('tracker_instance_id',instance,'task_id',task,'root_task_id',task,
        'project_id',i.project_id,'owner_subject',owner,'stage',stage,'status',status,
        'confirmation_revision',null,'assignment',null,'questions','[]'::jsonb,
        'revisions','[]'::jsonb,'confirmations','[]'::jsonb,'evidence','[]'::jsonb,'state_origin','manual_core');
    INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state,manual_control)
        VALUES(task,instance,i.project_id,task,owner,snapshot,true);
    UPDATE issues SET status_id=task_lifecycle_status_id(stage,status) WHERE id=task;
    INSERT INTO task_lifecycle_history(task_id,actor_id,action,previous_state,next_state,idempotency_key,command_hash)
    VALUES(task,actor,CASE WHEN importing THEN 'imported' ELSE 'created' END,
        jsonb_build_object('status_id',i.status_id),jsonb_build_object('stage',stage,'status',status,'revision',1),
        'initialize',encode(sha256(convert_to(snapshot::text,'UTF8')),'hex'));
END $$;
CREATE FUNCTION task_lifecycle_ensure_issue() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN PERFORM task_lifecycle_initialize_issue(NEW.id); RETURN NULL; END $$;
CREATE CONSTRAINT TRIGGER task_lifecycle_ensure_issue AFTER INSERT ON issues
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION task_lifecycle_ensure_issue();


CREATE VIEW task_lifecycle_inventory AS
SELECT i.id AS task_id,i.key AS task_key,i.project_id,i.status_id AS legacy_status_id,i.deleted_at,
    t.core_revision,t.state->>'stage' AS existing_stage,t.state->>'status' AS existing_status,
    CASE WHEN t.task_id IS NOT NULL THEN
        task_lifecycle_effective_stage(t.state)
        ELSE CASE i.status_id
            WHEN '00000000-0000-0000-0000-000000000001'::uuid THEN 'Backlog'
            WHEN '00000000-0000-0000-0000-000000000002'::uuid THEN 'Development'
            WHEN '00000000-0000-0000-0000-000000000004'::uuid THEN 'Review'
            WHEN '00000000-0000-0000-0000-000000000003'::uuid THEN 'Deployment' END END AS target_stage,
    CASE WHEN t.task_id IS NOT NULL THEN task_lifecycle_effective_status(t.state)
        ELSE CASE i.status_id
            WHEN '00000000-0000-0000-0000-000000000002'::uuid THEN 'InProgress'
            WHEN '00000000-0000-0000-0000-000000000003'::uuid THEN 'Completed'
            WHEN '00000000-0000-0000-0000-000000000001'::uuid THEN 'Ready'
            WHEN '00000000-0000-0000-0000-000000000004'::uuid THEN 'Ready' END END AS target_status,
    CASE WHEN t.task_id IS NULL AND i.status_id NOT IN (
        '00000000-0000-0000-0000-000000000001'::uuid,'00000000-0000-0000-0000-000000000002'::uuid,
        '00000000-0000-0000-0000-000000000003'::uuid,'00000000-0000-0000-0000-000000000004'::uuid)
        THEN 'unknown_legacy_status'
        WHEN t.task_id IS NOT NULL AND (t.project_id<>i.project_id
            OR COALESCE(t.state->>'stage','') NOT IN ('Draft','Clarification','Backlog','Analysis','Architecture','Development','Review','Testing','Rework','Deployment')
            OR COALESCE(task_lifecycle_effective_status(t.state),'') NOT IN ('Ready','InProgress','Completed'))
        THEN 'incomplete_or_inconsistent_sdlc_state'
        WHEN t.task_id IS NOT NULL AND t.state ? 'lifecycle' AND (
            t.state->'lifecycle'->'stage' IS DISTINCT FROM t.state->'stage'
            OR t.state->'lifecycle'->'status' IS DISTINCT FROM t.state->'status')
        THEN 'conflicting_execution_state'
        WHEN t.task_id IS NOT NULL AND task_lifecycle_execution_busy(t.task_id)
        THEN 'execution_not_drained'
        ELSE NULL END AS blocker
FROM issues i LEFT JOIN sdlc_tasks t ON t.task_id=i.id;


CREATE TABLE task_lifecycle_board_limits (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    stage text NOT NULL CHECK(stage IN ('Draft','Backlog','Analysis','Architecture','Development','Review','Testing','Rework','Deployment')),
    wip_limit integer CHECK(wip_limit>=0),
    legacy_status_id uuid REFERENCES statuses(id) ON DELETE RESTRICT,
    PRIMARY KEY(project_id,stage)
);

CREATE FUNCTION task_lifecycle_inventory_snapshot() RETURNS jsonb LANGUAGE sql STABLE AS $$
    SELECT COALESCE(jsonb_agg(to_jsonb(i) ORDER BY task_id),'[]'::jsonb) FROM task_lifecycle_inventory i
$$;

-- Operator boundary: qualify all serving writers before calling this transaction.
-- Compare the exact preflight snapshot again while holding the writer barrier.
CREATE FUNCTION task_lifecycle_activate(actor uuid, expected_inventory jsonb) RETURNS jsonb
LANGUAGE plpgsql AS $$
DECLARE snapshot jsonb; item record; previous uuid;
BEGIN
    LOCK TABLE issues,sdlc_tasks,boards IN SHARE ROW EXCLUSIVE MODE NOWAIT;
    PERFORM 1 FROM task_lifecycle_control WHERE singleton FOR UPDATE;
    IF (SELECT enabled FROM task_lifecycle_control WHERE singleton) THEN
        RETURN jsonb_build_object('enabled',true,'already_active',true);
    END IF;
    IF NOT EXISTS(SELECT 1 FROM users WHERE id=actor AND is_active) THEN
        RAISE EXCEPTION 'task_lifecycle_import_actor_required' USING ERRCODE='23514';
    END IF;
    IF NOT EXISTS(SELECT 1 FROM sdlc_instance WHERE singleton) THEN
        RAISE EXCEPTION 'task_model_instance_identity_required' USING ERRCODE='23514';
    END IF;
    snapshot:=task_lifecycle_inventory_snapshot();
    IF expected_inventory IS DISTINCT FROM snapshot THEN
        RAISE EXCEPTION 'task_lifecycle_inventory_changed' USING ERRCODE='23514';
    END IF;
    IF EXISTS(SELECT 1 FROM task_lifecycle_inventory WHERE blocker IS NOT NULL) THEN
        RAISE EXCEPTION 'task_lifecycle_inventory_blocked' USING ERRCODE='23514';
    END IF;
    -- Preserve all historical definitions. Only the shared compatibility projection changes.
    UPDATE sdlc_issue_status_mapping SET status_id=task_lifecycle_status_id(stage,status);
    -- The new projection guard is part of the same atomic cutover. Business stage
    -- projection no longer demands an agent confirmation; execution writers retain theirs.
    UPDATE task_lifecycle_control SET enabled=true WHERE singleton;
    FOR item IN SELECT * FROM task_lifecycle_inventory ORDER BY task_id LOOP
        IF item.core_revision IS NULL THEN
            PERFORM task_lifecycle_initialize_issue(item.task_id,true,actor);
        ELSE
            SELECT status_id INTO previous FROM issues WHERE id=item.task_id;
            UPDATE issues SET status_id=task_lifecycle_status_id(item.target_stage,item.target_status) WHERE id=item.task_id;
            INSERT INTO task_lifecycle_history(task_id,actor_id,action,previous_state,next_state,idempotency_key,command_hash)
            VALUES(item.task_id,actor,'imported',jsonb_build_object('status_id',previous),
                jsonb_build_object('stage',item.target_stage,'status',item.target_status,'revision',item.core_revision),
                'initialize',encode(sha256(convert_to(to_jsonb(item)::text,'UTF8')),'hex')) ON CONFLICT DO NOTHING;
        END IF;
    END LOOP;
    INSERT INTO task_lifecycle_board_limits(project_id,stage,wip_limit,legacy_status_id)
    SELECT b.project_id,CASE c->>'id'
            WHEN '00000000-0000-0000-0000-000000000001' THEN 'Backlog'
            WHEN '00000000-0000-0000-0000-000000000002' THEN 'Development'
            WHEN '00000000-0000-0000-0000-000000000004' THEN 'Review'
            WHEN '00000000-0000-0000-0000-000000000003' THEN 'Deployment' END,
        (c->>'wip_limit')::integer,(c->>'id')::uuid
    FROM boards b JOIN projects p ON p.default_board_id=b.id
    CROSS JOIN LATERAL jsonb_array_elements(b.columns::jsonb) c
    WHERE c->>'id' IN ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002',
        '00000000-0000-0000-0000-000000000003','00000000-0000-0000-0000-000000000004')
        AND c->>'wip_limit' IS NOT NULL ON CONFLICT DO NOTHING;
    UPDATE task_lifecycle_control SET enabled=true WHERE singleton;
    RETURN jsonb_build_object('enabled',true,'already_active',false,'tasks',jsonb_array_length(snapshot));
END $$;


CREATE FUNCTION task_lifecycle_capacity_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE target_stage text; cap integer; occupied bigint;
BEGIN
    IF NOT (SELECT enabled FROM task_lifecycle_control WHERE singleton) THEN RETURN NEW; END IF;
    target_stage:=CASE WHEN NEW.state->>'stage'='Clarification' THEN 'Draft' ELSE NEW.state->>'stage' END;
    IF TG_OP='UPDATE' AND NEW.state->'stage' IS NOT DISTINCT FROM OLD.state->'stage' THEN RETURN NEW; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended('pdlc.task-lifecycle.capacity/'||NEW.project_id::text||'/'||target_stage,0));
    SELECT wip_limit INTO cap FROM task_lifecycle_board_limits WHERE project_id=NEW.project_id AND task_lifecycle_board_limits.stage=target_stage;
    IF cap IS NULL THEN RETURN NEW; END IF;
    SELECT count(*) INTO occupied FROM sdlc_tasks t JOIN issues i ON i.id=t.task_id
        WHERE t.project_id=NEW.project_id AND t.task_id<>NEW.task_id AND i.deleted_at IS NULL
            AND (task_lifecycle_effective_stage(t.state))=target_stage;
    IF occupied>=cap THEN RAISE EXCEPTION 'task_lifecycle_wip_limit' USING ERRCODE='23514'; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER task_lifecycle_capacity_guard BEFORE INSERT OR UPDATE ON sdlc_tasks
FOR EACH ROW EXECUTE FUNCTION task_lifecycle_capacity_guard();


CREATE FUNCTION task_lifecycle_board_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE original jsonb; canonical jsonb; existing_ids jsonb;
BEGIN
    IF NOT (SELECT enabled FROM task_lifecycle_control WHERE singleton) THEN RETURN NEW; END IF;
    IF TG_OP='UPDATE' AND NEW.columns::jsonb IS NOT DISTINCT FROM OLD.columns::jsonb THEN RETURN NEW; END IF;
    original:=NEW.columns::jsonb;
    IF jsonb_typeof(original)<>'array' THEN RAISE EXCEPTION 'task_lifecycle_canonical_board_required' USING ERRCODE='23514'; END IF;
    SELECT jsonb_agg(c->>'id' ORDER BY c->>'id') INTO existing_ids FROM jsonb_array_elements(original) c;
    IF TG_OP='INSERT' AND jsonb_array_length(original) IN (0,4) THEN
        IF jsonb_array_length(original)=4 AND existing_ids IS DISTINCT FROM jsonb_build_array(
            '00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002',
            '00000000-0000-0000-0000-000000000003','00000000-0000-0000-0000-000000000004') THEN
            RAISE EXCEPTION 'task_lifecycle_canonical_board_required' USING ERRCODE='23514'; END IF;
    ELSIF jsonb_array_length(original)<>9 OR existing_ids IS DISTINCT FROM (
        SELECT jsonb_agg(task_lifecycle_status_id(stage,'Ready')::text ORDER BY task_lifecycle_status_id(stage,'Ready')::text)
        FROM sdlc_issue_status_mapping WHERE status='Ready') THEN
        RAISE EXCEPTION 'task_lifecycle_canonical_board_required' USING ERRCODE='23514';
    END IF;
    SELECT jsonb_agg(jsonb_build_object('id',task_lifecycle_status_id(stage,'Ready'),'name',stage,
        'category',CASE WHEN stage IN ('Draft','Backlog') THEN 'Todo' ELSE 'InProgress' END,
        'position',ordinal,'wip_limit',(SELECT c->'wip_limit' FROM jsonb_array_elements(original) c
            WHERE c->>'id'=task_lifecycle_status_id(stage,'Ready')::text OR c->>'id'=CASE stage
                WHEN 'Backlog' THEN '00000000-0000-0000-0000-000000000001'
                WHEN 'Development' THEN '00000000-0000-0000-0000-000000000002'
                WHEN 'Review' THEN '00000000-0000-0000-0000-000000000004'
                WHEN 'Deployment' THEN '00000000-0000-0000-0000-000000000003' END)) ORDER BY ordinal)
        INTO canonical FROM (VALUES('Draft',0),('Backlog',1),('Analysis',2),('Architecture',3),
            ('Development',4),('Review',5),('Testing',6),('Rework',7),('Deployment',8)) stages(stage,ordinal);
    NEW.columns:=canonical::json;
    RETURN NEW;
END $$;
CREATE TRIGGER task_lifecycle_board_guard BEFORE INSERT OR UPDATE ON boards
FOR EACH ROW EXECUTE FUNCTION task_lifecycle_board_guard();

CREATE FUNCTION task_lifecycle_board_capacity() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE column_value jsonb; stage_name text;
BEGIN
    IF TG_OP='UPDATE' AND NEW.columns::jsonb IS NOT DISTINCT FROM OLD.columns::jsonb THEN RETURN NULL; END IF;
    IF NOT (SELECT enabled FROM task_lifecycle_control WHERE singleton)
        OR (SELECT default_board_id FROM projects WHERE id=NEW.project_id) IS DISTINCT FROM NEW.id THEN RETURN NULL; END IF;
    FOR column_value IN SELECT value FROM jsonb_array_elements(NEW.columns::jsonb) LOOP
        SELECT stage INTO STRICT stage_name FROM sdlc_issue_status_mapping
            WHERE status='Ready' AND status_id=(column_value->>'id')::uuid;
        PERFORM pg_advisory_xact_lock(hashtextextended('pdlc.task-lifecycle.capacity/'||NEW.project_id::text||'/'||stage_name,0));
        INSERT INTO task_lifecycle_board_limits(project_id,stage,wip_limit)
            VALUES(NEW.project_id,stage_name,(column_value->>'wip_limit')::integer)
            ON CONFLICT(project_id,stage) DO UPDATE SET wip_limit=EXCLUDED.wip_limit;
    END LOOP;
    RETURN NULL;
END $$;
CREATE TRIGGER task_lifecycle_board_capacity AFTER INSERT OR UPDATE OF columns ON boards
FOR EACH ROW EXECUTE FUNCTION task_lifecycle_board_capacity();


CREATE FUNCTION task_lifecycle_activation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.enabled AND NOT NEW.enabled THEN
        RAISE EXCEPTION 'task_lifecycle_compatible_rollback_required' USING ERRCODE='23514';
    END IF;
    IF NEW.enabled AND NOT OLD.enabled THEN
        PERFORM pg_notify('pdlc_task_lifecycle','{"type":"task_lifecycle_activated"}');
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER task_lifecycle_activation_guard BEFORE UPDATE ON task_lifecycle_control
FOR EACH ROW EXECUTE FUNCTION task_lifecycle_activation_guard();


-- Called only by the startup adapter that observed an uninitialized database
-- before running migrations. Existing installations use the inventory/drain gate.
CREATE FUNCTION task_lifecycle_bootstrap_empty() RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    LOCK TABLE issues,sdlc_tasks,boards IN SHARE ROW EXCLUSIVE MODE;
    PERFORM 1 FROM task_lifecycle_control WHERE singleton FOR UPDATE;
    IF (SELECT enabled FROM task_lifecycle_control WHERE singleton) THEN RETURN; END IF;
    IF EXISTS(SELECT 1 FROM issues) OR EXISTS(SELECT 1 FROM sdlc_tasks) THEN
        RAISE EXCEPTION 'task_lifecycle_filled_installation_requires_import' USING ERRCODE='23514';
    END IF;
    IF NOT EXISTS(SELECT 1 FROM sdlc_instance WHERE singleton) THEN
        RAISE EXCEPTION 'task_model_instance_identity_required' USING ERRCODE='23514';
    END IF;
    UPDATE sdlc_issue_status_mapping SET status_id=task_lifecycle_status_id(stage,status);
    UPDATE task_lifecycle_control SET enabled=true WHERE singleton;
END $$;

-- Historical execution writers retain their original bodies and fencing checks.
-- The common guard validates the manual write, identity and custody snapshot independently.
DO $$ DECLARE name text; definition text; BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_pm_reservation_gate','sdlc_delivery_canonical_gate',
        'sdlc_configured_cli_completion_transition'] LOOP
        definition:=pg_get_functiondef(to_regprocedure(name||'()'));
        IF definition IS NULL OR position('BEGIN' IN definition)=0 THEN
            RAISE EXCEPTION 'task_lifecycle_expected_execution_guard_missing:%',name;
        END IF;
        definition:=overlay(definition placing E'BEGIN\n    IF TG_OP=''UPDATE'' AND task_lifecycle_manual_write_permitted(OLD.task_id,NEW.state) THEN RETURN NEW; END IF;'
            from position('BEGIN' IN definition) for 5);
        EXECUTE definition;
    END LOOP;
END $$;
CREATE OR REPLACE VIEW sdlc_delivery_tasks AS SELECT t.task_id,
    jsonb_build_object('task',t.state->'lifecycle') || h.execution AS receipt
    FROM sdlc_tasks t JOIN sdlc_delivery_heads h USING(task_id) WHERE NOT t.manual_control;


-- The existing explicit permanent-delete command may purge an ordinary core-only
-- task. Real execution/custody history retains its previous non-purgeable policy.
CREATE TABLE task_lifecycle_purge_permits (
    task_id uuid NOT NULL, transaction_id xid8 NOT NULL,
    PRIMARY KEY(task_id,transaction_id)
);
CREATE FUNCTION task_lifecycle_history_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' AND EXISTS(SELECT 1 FROM task_lifecycle_purge_permits p
        JOIN issues i ON i.id=p.task_id AND i.deleted_at IS NOT NULL
        WHERE p.task_id=OLD.task_id AND p.transaction_id=pg_current_xact_id()) THEN RETURN OLD; END IF;
    RAISE EXCEPTION 'task_lifecycle_history_immutable' USING ERRCODE='23514';
END $$;
DROP TRIGGER task_lifecycle_history_immutable ON task_lifecycle_history;
CREATE TRIGGER task_lifecycle_history_immutable BEFORE UPDATE OR DELETE ON task_lifecycle_history
FOR EACH ROW EXECUTE FUNCTION task_lifecycle_history_guard();

CREATE FUNCTION task_lifecycle_purge_manual_core(task uuid) RETURNS void LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks; trashed timestamptz;
BEGIN
    SELECT deleted_at INTO trashed FROM issues WHERE id=task FOR UPDATE;
    IF NOT FOUND OR trashed IS NULL THEN
        RAISE EXCEPTION 'task_lifecycle_trashed_issue_required' USING ERRCODE='23514';
    END IF;
    -- Preserve the existing project's DELETE authority and original commit deadline.
    IF ((SELECT active FROM tracker_space_admission_state WHERE singleton)
        OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL))
        AND NOT tracker_is_issue_purge_child('issue_status_history','DELETE',jsonb_build_object('issue_id',task)) THEN
        RAISE EXCEPTION 'task_lifecycle_purge_authority_required' USING ERRCODE='42501';
    END IF;
    SELECT * INTO t FROM sdlc_tasks WHERE task_id=task FOR UPDATE;
    IF NOT FOUND THEN RETURN; END IF;
    IF NOT t.manual_control OR t.state->>'state_origin' IS DISTINCT FROM 'manual_core'
        OR t.pm_execution_id IS NOT NULL OR COALESCE(t.state->'assignment','null'::jsonb)<>'null'::jsonb
        OR t.state ? 'flow' OR task_lifecycle_execution_busy(task)
        OR EXISTS(SELECT 1 FROM task_lifecycle_custody_history WHERE task_id=task) THEN
        RAISE EXCEPTION 'task_lifecycle_execution_history_retained' USING ERRCODE='23514';
    END IF;
    INSERT INTO task_lifecycle_purge_permits VALUES(task,pg_current_xact_id());
    DELETE FROM task_lifecycle_history WHERE task_id=task;
    DELETE FROM task_lifecycle_events WHERE task_id=task;
    DELETE FROM sdlc_tasks WHERE task_id=task;
    DELETE FROM task_lifecycle_purge_permits WHERE task_id=task AND transaction_id=pg_current_xact_id();
END $$;
