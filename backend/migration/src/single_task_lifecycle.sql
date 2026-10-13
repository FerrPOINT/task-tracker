-- Parent owns migration version/order/registration and legacy stage projection.
CREATE OR REPLACE FUNCTION sdlc_routing_snapshot_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM sdlc_tasks t JOIN sdlc_confirmations c ON c.task_id=t.task_id
        WHERE t.task_id=NEW.task_id AND t.project_id=NEW.project_id AND t.root_task_id=t.task_id
        AND t.tracker_instance_id=NEW.payload->>'tracker_instance_id'
        AND t.state->>'stage' IN ('Draft','Clarification')
        AND (t.pm_execution_id IS NULL
            OR sdlc_current_pm_draft_reserved(t.task_id,t.pm_execution_id,t.state,t.pm_owner_version,t.pm_admission_state)
            OR sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state))
        AND c.id=NEW.confirmation_id AND c.revision=NEW.requirement_revision AND c.content_hash=NEW.content_hash
        AND c.revision=(t.state->'revisions'->-1->>'revision')::bigint
        AND c.content_hash=t.state->'revisions'->-1->>'content_hash'
        AND c.payload->>'owner_subject'=t.owner_subject) OR NOT EXISTS (
        SELECT 1 FROM sdlc_project_routing_revisions r JOIN sdlc_project_routing_heads h USING(project_id)
        WHERE r.project_id=NEW.project_id AND r.version=NEW.policy_version AND h.version=r.version
        AND r.payload=NEW.payload->'policy') THEN
        RAISE EXCEPTION 'SDLC routing snapshot requires exact owner publication and current policy' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE TABLE sdlc_pm_feedback (
    message_id uuid PRIMARY KEY CHECK(message_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    actor_subject text NOT NULL,
    payload jsonb NOT NULL,
    CHECK ((payload->>'message_id'=message_id::text AND payload->>'task_id'=task_id::text
        AND payload->>'root_task_id'=task_id::text AND payload->>'owner_subject'=actor_subject
        AND octet_length(payload->>'text') BETWEEN 1 AND 100000
        AND payload->>'content_hash' ~ '^[0-9a-f]{64}$') IS TRUE)
);
CREATE INDEX sdlc_pm_feedback_task ON sdlc_pm_feedback(task_id);
CREATE TRIGGER sdlc_history_immutable BEFORE UPDATE OR DELETE ON sdlc_pm_feedback
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE TABLE sdlc_delivery_heads (
    task_id uuid PRIMARY KEY REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    execution jsonb NOT NULL CHECK (jsonb_typeof(execution)='object' AND NOT execution ? 'task')
);
CREATE VIEW sdlc_delivery_tasks AS SELECT t.task_id,
    jsonb_build_object('task',t.state->'lifecycle') || h.execution AS receipt
    FROM sdlc_tasks t JOIN sdlc_delivery_heads h USING(task_id);
CREATE TABLE sdlc_delivery_projection_permits (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    transaction_id xid8 NOT NULL, next_state jsonb NOT NULL, next_execution jsonb NOT NULL,
    PRIMARY KEY(task_id,transaction_id)
);
ALTER TABLE sdlc_tasks DROP CONSTRAINT sdlc_tasks_stage_known;
ALTER TABLE sdlc_tasks ADD CONSTRAINT sdlc_tasks_stage_known CHECK
    (state->>'stage' IN ('Draft','Clarification','Backlog','Analysis','Architecture','Development','Review','Testing','Rework','Deployment'));
ALTER TABLE sdlc_tasks ADD CONSTRAINT sdlc_status_known CHECK
    (NOT state ? 'status' OR (state->>'status' IN ('Ready','InProgress','Completed')) IS TRUE);
ALTER TABLE sdlc_tasks ADD CONSTRAINT sdlc_canonical_lifecycle CHECK (NOT state ? 'lifecycle' OR
    (state->'lifecycle'->>'task_id'=task_id::text AND state->'lifecycle'->>'owner_subject'=owner_subject
        AND state->'lifecycle'->'stage'=state->'stage' AND state->'lifecycle'->'status'=state->'status'
        AND state->'lifecycle'->'waiting_reason'=state->'waiting_reason'
        AND state->'lifecycle'->'runtime'=state->'runtime_state'
        AND state->>'status' IN ('Ready','InProgress','Completed')
        AND (state->'lifecycle'->>'version')::bigint BETWEEN 1 AND 9007199254740991
        AND (state->'lifecycle'->>'cycle')::integer BETWEEN 0 AND 3
        AND (state->'lifecycle'->>'transient_retries')::integer BETWEEN 0 AND 3
        AND state->'lifecycle'->'routing'->>'root_task_id'=task_id::text
        AND state->'lifecycle'->'routing'->>'project_id'=project_id::text) IS TRUE);
INSERT INTO statuses(id,name,category,position,is_default,is_closed)
SELECT gen_random_uuid(),'SDLC '||status,category,0,false,status='Completed'
FROM (VALUES('Ready','todo'),('InProgress','in_progress'),('Completed','done')) phases(status,category)
WHERE NOT EXISTS(SELECT 1 FROM statuses s WHERE s.name='SDLC '||status);
CREATE TABLE sdlc_issue_status_mapping (
    stage text NOT NULL CHECK(stage IN ('Draft','Backlog','Analysis','Architecture','Development','Review','Testing','Rework','Deployment')),
    status text NOT NULL CHECK(status IN ('Ready','InProgress','Completed')),
    status_id uuid NOT NULL REFERENCES statuses(id) ON DELETE RESTRICT,
    PRIMARY KEY(stage,status)
);
INSERT INTO sdlc_issue_status_mapping(stage,status,status_id)
SELECT stage,status,s.id
FROM (VALUES('Draft'),('Backlog'),('Analysis'),('Architecture'),('Development'),('Review'),('Testing'),('Rework'),('Deployment')) stages(stage)
CROSS JOIN (VALUES('Ready','todo'),('InProgress','in_progress'),('Completed','done')) phases(status,category)
JOIN statuses s ON s.name='SDLC '||status AND s.category=phases.category AND s.is_closed=(status='Completed');

CREATE TABLE sdlc_delivery_assignments (
    assignment_id uuid PRIMARY KEY CHECK(assignment_id<>'00000000-0000-0000-0000-000000000000'),
    execution_id uuid NOT NULL UNIQUE CHECK(execution_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    agent_id uuid NOT NULL CHECK(agent_id<>'00000000-0000-0000-0000-000000000000'),
    payload jsonb NOT NULL,
    UNIQUE(assignment_id,task_id,agent_id),
    CHECK ((payload->>'assignment_id'=assignment_id::text
        AND payload->>'execution_id'=execution_id::text AND payload->>'task_id'=task_id::text
        AND payload->>'agent_id'=agent_id::text
        AND payload->>'assignment_hash' ~ '^[0-9a-f]{64}$'
        AND (payload->>'fencing_token')::bigint BETWEEN 1 AND 9007199254740991
        AND payload->'fencing_token'=to_jsonb((payload->>'fencing_token')::bigint)
        AND (payload->>'cycle')::integer BETWEEN 0 AND 3
        AND (payload->>'attempt')::integer BETWEEN 1 AND 4) IS TRUE)
);
CREATE INDEX sdlc_delivery_assignment_task ON sdlc_delivery_assignments(task_id);

CREATE TABLE sdlc_delivery_holds (
    assignment_id uuid PRIMARY KEY,
    task_id uuid NOT NULL UNIQUE,
    agent_id uuid NOT NULL UNIQUE,
    pool_slot smallint NOT NULL UNIQUE CHECK(pool_slot BETWEEN 1 AND 2),
    FOREIGN KEY(assignment_id,task_id,agent_id)
        REFERENCES sdlc_delivery_assignments(assignment_id,task_id,agent_id) ON DELETE RESTRICT
);
CREATE VIEW sdlc_shared_capacity AS SELECT root_task_id AS task_id,agent_id,pool_slot
    FROM sdlc_analysis_capacity_holds UNION ALL SELECT task_id,agent_id,pool_slot FROM sdlc_delivery_holds;

CREATE TABLE sdlc_delivery_run_bindings (
    assignment_id uuid PRIMARY KEY REFERENCES sdlc_delivery_assignments(assignment_id) ON DELETE RESTRICT,
    run_id uuid NOT NULL UNIQUE CHECK(run_id<>'00000000-0000-0000-0000-000000000000'),
    chat_id uuid NOT NULL CHECK(chat_id<>'00000000-0000-0000-0000-000000000000'),
    payload jsonb NOT NULL,
    CHECK ((payload->>'run_id'=run_id::text AND payload->>'chat_id'=chat_id::text
        AND payload->'dispatch_receipt'->>'sha256' ~ '^[0-9a-f]{64}$'
        AND octet_length(payload->'dispatch_receipt'->>'reference') BETWEEN 1 AND 512) IS TRUE)
);
CREATE TRIGGER sdlc_delivery_hold_no_update BEFORE UPDATE ON sdlc_delivery_holds
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
CREATE TRIGGER sdlc_delivery_head_no_delete BEFORE DELETE ON sdlc_delivery_heads
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE TABLE sdlc_delivery_proposals (
    proposal_id uuid PRIMARY KEY,
    assignment_id uuid NOT NULL UNIQUE REFERENCES sdlc_delivery_run_bindings(assignment_id) ON DELETE RESTRICT,
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    payload jsonb NOT NULL,
    CHECK ((payload->>'proposal_id'=proposal_id::text AND payload->>'assignment_id'=assignment_id::text
        AND payload->>'task_id'=task_id::text AND payload->>'output_sha256' ~ '^[0-9a-f]{64}$'
        AND payload->'workflow_receipt'->>'sha256' ~ '^[0-9a-f]{64}$') IS TRUE)
);
CREATE INDEX sdlc_delivery_proposal_task ON sdlc_delivery_proposals(task_id);

CREATE TABLE sdlc_delivery_proposal_operations (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    payload_hash text NOT NULL CHECK(payload_hash ~ '^[0-9a-f]{64}$'),
    result jsonb NOT NULL,
    PRIMARY KEY(task_id,actor_subject,idempotency_key),
    CHECK ((result->>'task_id'=task_id::text AND result->>'producer_subject'=actor_subject) IS TRUE)
);

CREATE TABLE sdlc_delivery_results (
    result_id uuid PRIMARY KEY,
    assignment_id uuid NOT NULL UNIQUE REFERENCES sdlc_delivery_run_bindings(assignment_id) ON DELETE RESTRICT,
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    payload jsonb NOT NULL,
    CHECK ((payload->>'result_id'=result_id::text AND payload->>'assignment_id'=assignment_id::text
        AND payload->>'task_id'=task_id::text
        AND payload->'terminal'->>'output_sha256' ~ '^[0-9a-f]{64}$') IS TRUE)
);
CREATE INDEX sdlc_delivery_result_task ON sdlc_delivery_results(task_id);

CREATE TABLE sdlc_delivery_operations (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    payload_hash text NOT NULL CHECK(payload_hash ~ '^[0-9a-f]{64}$'),
    event_id uuid UNIQUE,
    result jsonb NOT NULL,
    PRIMARY KEY(task_id,actor_subject,idempotency_key),
    CHECK ((result->'task'->>'task_id'=task_id::text) IS TRUE)
);

CREATE TABLE sdlc_delivery_reconciliations (
    assignment_id uuid PRIMARY KEY REFERENCES sdlc_delivery_assignments(assignment_id) ON DELETE RESTRICT,
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    payload jsonb NOT NULL,
    CHECK ((payload->>'operation'='reconcile' AND payload->'fence'->>'assignment_id'=assignment_id::text
        AND payload->'terminal'->>'state' IN ('succeeded','failed','cancelled')
        AND payload->'terminal'->'receipt'->>'sha256' ~ '^[0-9a-f]{64}$') IS TRUE)
);
CREATE INDEX sdlc_delivery_reconciliation_task ON sdlc_delivery_reconciliations(task_id);

DO $$ DECLARE name text; BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_delivery_assignments','sdlc_delivery_run_bindings',
        'sdlc_delivery_results','sdlc_delivery_operations','sdlc_delivery_proposals',
        'sdlc_delivery_proposal_operations','sdlc_delivery_reconciliations'] LOOP
        EXECUTE format('CREATE TRIGGER sdlc_history_immutable BEFORE UPDATE OR DELETE ON %I
            FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable()', name);
    END LOOP;
END $$;

CREATE FUNCTION sdlc_delivery_capacity_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds
        WHERE root_task_id=NEW.task_id OR agent_id=NEW.agent_id OR pool_slot=NEW.pool_slot)
        OR EXISTS(SELECT 1 FROM sdlc_assignments a JOIN sdlc_tasks t ON t.task_id=a.task_id
            AND t.pm_execution_id=(a.payload->>'execution_id')::uuid
            WHERE a.payload->>'agent_id'=NEW.agent_id::text
                AND NOT sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)) THEN
        RAISE EXCEPTION 'SDLC previous execution capacity held' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_capacity_gate BEFORE INSERT ON sdlc_delivery_holds
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_capacity_gate();

CREATE FUNCTION sdlc_delivery_legacy_exclusion() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE selected_agent text; selected_task uuid;
BEGIN
    selected_task := NEW.task_id;
    selected_agent := NEW.payload->>'agent_id';
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_delivery_tasks WHERE task_id=selected_task)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE agent_id::text=selected_agent) THEN
        RAISE EXCEPTION 'SDLC single-task delivery owns execution' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_legacy_exclusion BEFORE INSERT ON sdlc_analysis_reservations
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_legacy_exclusion();
CREATE TRIGGER sdlc_delivery_legacy_exclusion BEFORE INSERT ON sdlc_assignments
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_legacy_exclusion();

CREATE FUNCTION sdlc_delivery_legacy_slot_exclusion() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_delivery_holds
        WHERE task_id=NEW.root_task_id OR agent_id=NEW.agent_id OR pool_slot=NEW.pool_slot) THEN
        RAISE EXCEPTION 'SDLC delivery capacity held' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_legacy_slot_exclusion BEFORE INSERT ON sdlc_analysis_capacity_holds
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_legacy_slot_exclusion();

CREATE FUNCTION sdlc_delivery_hold_release_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_results WHERE assignment_id=OLD.assignment_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_reconciliations WHERE assignment_id=OLD.assignment_id) THEN
        RAISE EXCEPTION 'SDLC terminal receipt required before capacity release' USING ERRCODE='23514';
    END IF;
    RETURN OLD;
END $$;
CREATE TRIGGER sdlc_delivery_hold_release_gate BEFORE DELETE ON sdlc_delivery_holds
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_hold_release_gate();

CREATE FUNCTION sdlc_delivery_assignment_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb; task sdlc_tasks%ROWTYPE;
BEGIN
    SELECT * INTO task FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    IF (head->'task'->>'status'='Ready' AND head->'task'->>'runtime'='queued'
        AND head->'assignment'='null'::jsonb
        AND NEW.payload->>'stage'=head->'task'->>'stage'
        AND NEW.payload->'cycle'=head->'task'->'cycle'
        AND NEW.payload->'routing'=head->'task'->'routing'
        AND NEW.payload->'previous_result_id'=head->'task'->'previous_result_id'
        AND NEW.payload->>'owner_subject'=task.owner_subject
        AND NEW.agent_id::text=head->'task'->'routing'->'policy'->'routes'->(NEW.payload->>'role')->>'agent_id'
        AND NEW.payload->>'role'=CASE head->'task'->>'stage'
            WHEN 'Analysis' THEN 'analyst' WHEN 'Architecture' THEN 'architect'
            WHEN 'Development' THEN 'developer' WHEN 'Rework' THEN 'developer'
            WHEN 'Review' THEN 'reviewer' WHEN 'Testing' THEN 'tester'
            WHEN 'Deployment' THEN 'devops' END) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC assignment requires exact queued role and frozen routing' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_assignment_gate BEFORE INSERT ON sdlc_delivery_assignments
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_assignment_gate();

CREATE FUNCTION sdlc_delivery_run_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb;
BEGIN
    SELECT t.receipt INTO head FROM sdlc_delivery_tasks t
        JOIN sdlc_delivery_assignments a ON a.task_id=t.task_id
        WHERE a.assignment_id=NEW.assignment_id FOR UPDATE OF t;
    IF (head->'assignment'->>'assignment_id'=NEW.assignment_id::text
        AND head->'task'->>'runtime'='reserved' AND head->'binding'='null'::jsonb
        AND (head->'lease'->>'expires_at')::timestamptz>clock_timestamp()) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC run binding requires current live reserved assignment' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_run_gate BEFORE INSERT ON sdlc_delivery_run_bindings
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_run_gate();

CREATE FUNCTION sdlc_delivery_proposal_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb; binding jsonb;
BEGIN
    SELECT t.receipt,b.payload INTO head,binding FROM sdlc_delivery_tasks t
        JOIN sdlc_delivery_assignments a ON a.task_id=t.task_id
        JOIN sdlc_delivery_run_bindings b USING(assignment_id)
        JOIN sdlc_delivery_holds h USING(assignment_id)
        WHERE t.task_id=NEW.task_id AND a.assignment_id=NEW.assignment_id FOR UPDATE OF t;
    IF (head->'assignment'->>'assignment_id'=NEW.assignment_id::text
        AND head->'task'->>'runtime'='running' AND head->'task'->>'status'='InProgress'
        AND (head->'lease'->>'expires_at')::timestamptz>clock_timestamp()
        AND NEW.payload->>'producer_subject'=binding->>'machine_subject'
        AND NEW.payload->>'execution_id'=head->'assignment'->>'execution_id'
        AND NEW.payload->>'agent_id'=head->'assignment'->>'agent_id'
        AND NEW.payload->'result'->>'role'=head->'assignment'->>'role') IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC proposal requires exact live producer assignment' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_proposal_gate BEFORE INSERT ON sdlc_delivery_proposals
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_proposal_gate();

CREATE FUNCTION sdlc_delivery_result_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb; binding jsonb; proposal jsonb;
    expected_stage text; expected_status text; expected_cycle integer; role text; verdict text;
BEGIN
    SELECT t.receipt,b.payload INTO head,binding FROM sdlc_delivery_tasks t
        JOIN sdlc_delivery_assignments a ON a.task_id=t.task_id
        JOIN sdlc_delivery_run_bindings b USING(assignment_id)
        JOIN sdlc_delivery_holds h USING(assignment_id)
        WHERE t.task_id=NEW.task_id AND a.assignment_id=NEW.assignment_id FOR UPDATE OF t;
    SELECT payload INTO proposal FROM sdlc_delivery_proposals WHERE assignment_id=NEW.assignment_id;
    role := head->'assignment'->>'role';
    verdict := NEW.payload->'output'->>'verdict';
    expected_cycle := (head->'task'->>'cycle')::integer;
    expected_status := CASE WHEN role='devops' THEN 'Completed' ELSE 'Ready' END;
    expected_stage := CASE role WHEN 'analyst' THEN 'Architecture' WHEN 'architect' THEN 'Development'
        WHEN 'developer' THEN 'Review' WHEN 'reviewer' THEN 'Testing'
        WHEN 'tester' THEN 'Deployment' WHEN 'devops' THEN 'Deployment' END;
    IF role IN ('reviewer','tester') THEN
        IF verdict='rework' THEN
            IF expected_cycle=3 THEN expected_stage := head->'task'->>'stage';
            ELSE expected_stage := 'Rework'; expected_cycle := expected_cycle+1; END IF;
        ELSIF (role='reviewer' AND verdict='approved') OR (role='tester' AND verdict='passed') THEN
            NULL;
        ELSE RAISE EXCEPTION 'SDLC invalid typed review/testing verdict' USING ERRCODE='23514'; END IF;
    END IF;
    IF (head->'assignment'->>'assignment_id'=NEW.assignment_id::text
        AND head->'task'->>'status'='InProgress' AND head->'task'->>'runtime'='running'
        AND (head->'lease'->>'expires_at')::timestamptz>clock_timestamp()
        AND head->'task'->>'stage'=NEW.payload->>'previous_stage'
        AND head->'assignment'->>'role'=NEW.payload->'output'->>'role'
        AND NEW.payload->'terminal'->>'run_id'=binding->>'run_id'
        AND NEW.payload->'terminal'->>'chat_id'=binding->>'chat_id'
        AND NEW.payload->'output'=proposal->'result'
        AND NEW.payload->'terminal'->'workflow_receipt'=proposal->'workflow_receipt'
        AND NEW.payload->'terminal'->'output_sha256'=proposal->'output_sha256'
        AND NEW.payload->>'next_stage'=expected_stage AND NEW.payload->>'next_status'=expected_status
        AND NEW.payload->'cycle'=to_jsonb(expected_cycle)) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC result requires exact live bound execution' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_result_gate BEFORE INSERT ON sdlc_delivery_results
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_result_gate();

CREATE FUNCTION sdlc_delivery_head_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task sdlc_tasks%ROWTYPE; previous_lease jsonb; next_lease jsonb;
BEGIN
    SELECT * INTO task FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p WHERE p.task_id=NEW.task_id
        AND p.transaction_id=pg_current_xact_id() AND p.next_state=task.state AND p.next_execution=NEW.execution) THEN
        RAISE EXCEPTION 'SDLC execution cursor requires canonical transaction permit' USING ERRCODE='23514';
    END IF;
    IF TG_OP='UPDATE' THEN
        IF NEW.task_id<>OLD.task_id THEN RAISE EXCEPTION 'SDLC immutable task identity' USING ERRCODE='23514'; END IF;
        IF NEW.execution->'assignment'='null'::jsonb AND OLD.execution->'assignment'<>'null'::jsonb THEN
            IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_results r WHERE r.task_id=NEW.task_id
                AND r.result_id::text=task.state->'lifecycle'->>'previous_result_id'
                AND r.assignment_id::text=OLD.execution->'assignment'->>'assignment_id')
                AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_reconciliations r WHERE r.task_id=NEW.task_id
                    AND r.assignment_id::text=OLD.execution->'assignment'->>'assignment_id') THEN
                RAISE EXCEPTION 'SDLC lifecycle transition requires accepted typed result' USING ERRCODE='23514';
            END IF;
        END IF;
        previous_lease := OLD.execution->'lease'; next_lease := NEW.execution->'lease';
        IF previous_lease<>'null'::jsonb AND next_lease<>'null'::jsonb
            AND previous_lease IS DISTINCT FROM next_lease THEN
            IF ((next_lease->>'version')::bigint=(previous_lease->>'version')::bigint+1
                AND (previous_lease->>'expires_at')::timestamptz>clock_timestamp()
                AND next_lease->'lease_id'=previous_lease->'lease_id'
                AND next_lease->'holder_subject'=previous_lease->'holder_subject'
                AND next_lease->'claimed_at'=previous_lease->'claimed_at'
                AND (next_lease->>'expires_at')::timestamptz=
                    (next_lease->>'heartbeat_at')::timestamptz+interval '30 seconds') IS NOT TRUE THEN
                RAISE EXCEPTION 'SDLC active lease CAS and TTL30 required' USING ERRCODE='23514';
            END IF;
        END IF;
    END IF;
    DELETE FROM sdlc_delivery_projection_permits WHERE task_id=NEW.task_id AND transaction_id=pg_current_xact_id();
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_head_gate BEFORE INSERT OR UPDATE ON sdlc_delivery_heads
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_head_gate();

CREATE FUNCTION sdlc_delivery_canonical_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE old_task jsonb; next_task jsonb;
BEGIN
    IF OLD.state ? 'status' AND NOT NEW.state ? 'status' THEN
        RAISE EXCEPTION 'SDLC canonical status cannot be dropped by legacy serialization' USING ERRCODE='23514';
    END IF;
    IF (OLD.state ? 'lifecycle' OR NEW.state ? 'lifecycle') AND OLD.state IS DISTINCT FROM NEW.state THEN
        IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p WHERE p.task_id=OLD.task_id
            AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN
            RAISE EXCEPTION 'SDLC canonical lifecycle requires exact transaction permit' USING ERRCODE='23514';
        END IF;
        next_task := NEW.state->'lifecycle'; old_task := OLD.state->'lifecycle';
        IF NOT EXISTS(SELECT 1 FROM sdlc_task_routing_snapshots s JOIN sdlc_confirmations c
            ON c.id=s.confirmation_id AND c.task_id=s.task_id
            WHERE s.task_id=OLD.task_id AND s.payload=next_task->'routing'
                AND s.requirement_revision=(NEW.state->'revisions'->-1->>'revision')::bigint
                AND s.content_hash=NEW.state->'revisions'->-1->>'content_hash'
                AND c.payload->>'owner_subject'=NEW.owner_subject AND NEW.root_task_id=NEW.task_id) THEN
            RAISE EXCEPTION 'SDLC exact frozen owner consent required' USING ERRCODE='23514';
        END IF;
        IF old_task IS NULL THEN
            IF (next_task->'version'='1'::jsonb AND next_task->>'status'='Ready'
                AND next_task->>'runtime'='queued' AND next_task->'cycle'='0'::jsonb
                AND NEW.state->'stage'=OLD.state->'stage'
                AND (OLD.state->>'stage'='Analysis' OR (OLD.state->>'stage'='Architecture'
                    AND EXISTS(SELECT 1 FROM sdlc_analysis_results r WHERE r.task_id=OLD.task_id
                        AND r.result_id::text=next_task->>'previous_result_id')))) IS NOT TRUE THEN
                RAISE EXCEPTION 'SDLC initial canonical queue mismatch' USING ERRCODE='23514';
            END IF;
        ELSE
            IF next_task->'routing' IS DISTINCT FROM old_task->'routing'
                OR (next_task->>'version')::bigint NOT BETWEEN (old_task->>'version')::bigint AND (old_task->>'version')::bigint+1 THEN
                RAISE EXCEPTION 'SDLC immutable routing or stale canonical version' USING ERRCODE='23514';
            END IF;
            IF next_task->'stage' IS DISTINCT FROM old_task->'stage'
                OR next_task->'cycle' IS DISTINCT FROM old_task->'cycle'
                OR next_task->'previous_result_id' IS DISTINCT FROM old_task->'previous_result_id' THEN
                IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_results r WHERE r.task_id=OLD.task_id
                    AND r.result_id::text=next_task->>'previous_result_id'
                    AND r.payload->'next_stage'=next_task->'stage' AND r.payload->'next_status'=next_task->'status'
                    AND r.payload->'cycle'=next_task->'cycle') THEN
                    RAISE EXCEPTION 'SDLC canonical transition requires accepted result' USING ERRCODE='23514';
                END IF;
            END IF;
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_canonical_gate BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_canonical_gate();

CREATE FUNCTION sdlc_delivery_reconciliation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb;
BEGIN
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    IF (head->'assignment'->>'assignment_id'=NEW.assignment_id::text
        AND head->'assignment'->'execution_id'=NEW.payload->'fence'->'execution_id'
        AND head->'assignment'->'fencing_token'=NEW.payload->'fence'->'fencing_token'
        AND head->'assignment'->'lease_id'=NEW.payload->'fence'->'lease_id'
        AND (NEW.payload->'fence'->>'expected_lease_version')::bigint BETWEEN 1 AND (head->'lease'->>'version')::bigint
        AND head->'assignment'->'assignment_hash'=NEW.payload->'terminal'->'assignment_hash'
        AND (head->'binding'='null'::jsonb OR
            (head->'binding'->'run_id'=NEW.payload->'terminal'->'run_id'
                AND head->'binding'->'chat_id'=NEW.payload->'terminal'->'chat_id'))) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC reconciliation requires exact actual terminal assignment/run' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_reconciliation_gate BEFORE INSERT ON sdlc_delivery_reconciliations
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_reconciliation_gate();

CREATE OR REPLACE FUNCTION sdlc_analysis_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state ? 'lifecycle' OR NEW.state ? 'lifecycle' THEN
        IF OLD.state IS DISTINCT FROM NEW.state AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p
            WHERE p.task_id=OLD.task_id AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN
            RAISE EXCEPTION 'SDLC Analysis requires canonical lifecycle permit' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.state->>'stage'='Analysis' AND NEW.state IS DISTINCT FROM OLD.state THEN
        IF NEW.state->>'stage'!='Architecture' OR NOT EXISTS(SELECT 1 FROM sdlc_analysis_state_write_permits p
            WHERE p.task_id=OLD.task_id AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state)
            OR NOT EXISTS(SELECT 1 FROM sdlc_analysis_results r WHERE r.task_id=OLD.task_id
                AND r.payload->>'requirement_revision'=OLD.state->'revisions'->-1->>'revision'
                AND r.payload->>'content_hash'=OLD.state->'revisions'->-1->>'content_hash') THEN
            RAISE EXCEPTION 'SDLC legacy Analysis requires exact accepted result and permit' USING ERRCODE='23514';
        END IF;
        DELETE FROM sdlc_analysis_state_write_permits WHERE task_id=OLD.task_id AND transaction_id=pg_current_xact_id();
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION sdlc_issue_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE binding sdlc_tasks%ROWTYPE; target_name text;
BEGIN
    SELECT * INTO binding FROM sdlc_tasks WHERE task_id=NEW.id;
    IF NOT FOUND THEN RETURN NEW; END IF;
    IF NEW.project_id<>binding.project_id THEN RAISE EXCEPTION 'SDLC immutable project binding' USING ERRCODE='23514'; END IF;
    SELECT name INTO target_name FROM statuses WHERE id=NEW.status_id;
    IF binding.state ? 'status' THEN
        IF NOT EXISTS(SELECT 1 FROM sdlc_issue_status_mapping m WHERE m.stage=binding.state->>'stage'
                AND m.status=binding.state->>'status' AND m.status_id=NEW.status_id)
            OR NEW.sprint_id IS NOT NULL OR (binding.state->>'stage' NOT IN ('Draft','Clarification') AND NOT EXISTS(SELECT 1 FROM sdlc_confirmations c WHERE c.task_id=NEW.id
                AND c.revision=(binding.state->'revisions'->-1->>'revision')::bigint
                AND c.content_hash=binding.state->'revisions'->-1->>'content_hash')) THEN
            RAISE EXCEPTION 'SDLC issue stage/status must match canonical confirmed lifecycle' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    IF binding.state->>'stage' IN ('Draft','Clarification') THEN
        IF target_name IS DISTINCT FROM 'SDLC '||(binding.state->>'stage') OR NEW.sprint_id IS NOT NULL THEN
            RAISE EXCEPTION 'SDLC confirmation gate blocks transition/sprint' USING ERRCODE='23514'; END IF;
    ELSIF NOT EXISTS(SELECT 1 FROM sdlc_confirmations c WHERE c.task_id=NEW.id
        AND c.revision=(binding.state->'revisions'->-1->>'revision')::bigint
        AND c.content_hash=binding.state->'revisions'->-1->>'content_hash') THEN
        RAISE EXCEPTION 'SDLC exact revision confirmation absent' USING ERRCODE='23514';
    END IF;
    IF binding.state->>'stage'='Analysis' AND (target_name IS DISTINCT FROM 'SDLC Analysis' OR NEW.sprint_id IS NOT NULL
        OR NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents q WHERE q.task_id=NEW.id
            AND q.requirement_revision=(binding.state->'revisions'->-1->>'revision')::bigint
            AND q.content_hash=binding.state->'revisions'->-1->>'content_hash')) THEN
        RAISE EXCEPTION 'SDLC legacy Analysis intent required' USING ERRCODE='23514';
    ELSIF binding.state->>'stage'='Architecture' AND (target_name IS DISTINCT FROM 'SDLC Architecture' OR NEW.sprint_id IS NOT NULL
        OR NOT EXISTS(SELECT 1 FROM sdlc_analysis_results r WHERE r.task_id=NEW.id
            AND r.payload->>'requirement_revision'=binding.state->'revisions'->-1->>'revision'
            AND r.payload->>'content_hash'=binding.state->'revisions'->-1->>'content_hash')) THEN
        RAISE EXCEPTION 'SDLC legacy Architecture terminal required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
