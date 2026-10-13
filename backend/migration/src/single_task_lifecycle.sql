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

CREATE TABLE sdlc_delivery_tasks (
    task_id uuid PRIMARY KEY REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    receipt jsonb NOT NULL,
    CHECK ((receipt->'task'->>'task_id'=task_id::text
        AND receipt->'task'->'version'=to_jsonb((receipt->'task'->>'version')::bigint)
        AND (receipt->'task'->>'version')::bigint BETWEEN 1 AND 9007199254740991
        AND receipt->'task'->>'stage' IN ('Analysis','Architecture','Development','Review','Testing','Rework','Deployment')
        AND receipt->'task'->>'status' IN ('Ready','InProgress','Completed')
        AND receipt->'task'->>'runtime' IN ('queued','reserved','running','terminal','unknown')
        AND (receipt->'task'->>'cycle')::integer BETWEEN 0 AND 3
        AND (receipt->'task'->>'transient_retries')::integer BETWEEN 0 AND 3
        AND receipt->'task'->'routing'->>'task_id'=task_id::text
        AND receipt->'task'->'routing'->>'root_task_id'=task_id::text) IS TRUE)
);

CREATE TABLE sdlc_delivery_assignments (
    assignment_id uuid PRIMARY KEY CHECK(assignment_id<>'00000000-0000-0000-0000-000000000000'),
    execution_id uuid NOT NULL UNIQUE CHECK(execution_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL REFERENCES sdlc_delivery_tasks(task_id) ON DELETE RESTRICT,
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
CREATE TRIGGER sdlc_delivery_head_no_delete BEFORE DELETE ON sdlc_delivery_tasks
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE TABLE sdlc_delivery_proposals (
    proposal_id uuid PRIMARY KEY,
    assignment_id uuid NOT NULL UNIQUE REFERENCES sdlc_delivery_run_bindings(assignment_id) ON DELETE RESTRICT,
    task_id uuid NOT NULL REFERENCES sdlc_delivery_tasks(task_id) ON DELETE RESTRICT,
    payload jsonb NOT NULL,
    CHECK ((payload->>'proposal_id'=proposal_id::text AND payload->>'assignment_id'=assignment_id::text
        AND payload->>'task_id'=task_id::text AND payload->>'output_sha256' ~ '^[0-9a-f]{64}$'
        AND payload->'workflow_receipt'->>'sha256' ~ '^[0-9a-f]{64}$') IS TRUE)
);
CREATE INDEX sdlc_delivery_proposal_task ON sdlc_delivery_proposals(task_id);

CREATE TABLE sdlc_delivery_proposal_operations (
    task_id uuid NOT NULL REFERENCES sdlc_delivery_tasks(task_id) ON DELETE RESTRICT,
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
    task_id uuid NOT NULL REFERENCES sdlc_delivery_tasks(task_id) ON DELETE RESTRICT,
    payload jsonb NOT NULL,
    CHECK ((payload->>'result_id'=result_id::text AND payload->>'assignment_id'=assignment_id::text
        AND payload->>'task_id'=task_id::text
        AND payload->'terminal'->>'output_sha256' ~ '^[0-9a-f]{64}$') IS TRUE)
);
CREATE INDEX sdlc_delivery_result_task ON sdlc_delivery_results(task_id);

CREATE TABLE sdlc_delivery_operations (
    task_id uuid NOT NULL REFERENCES sdlc_delivery_tasks(task_id) ON DELETE RESTRICT,
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    payload_hash text NOT NULL CHECK(payload_hash ~ '^[0-9a-f]{64}$'),
    result jsonb NOT NULL,
    PRIMARY KEY(task_id,actor_subject,idempotency_key),
    CHECK ((result->'task'->>'task_id'=task_id::text) IS TRUE)
);

DO $$ DECLARE name text; BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_delivery_assignments','sdlc_delivery_run_bindings',
        'sdlc_delivery_results','sdlc_delivery_operations','sdlc_delivery_proposals',
        'sdlc_delivery_proposal_operations'] LOOP
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
    IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_results WHERE assignment_id=OLD.assignment_id) THEN
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
    IF NOT EXISTS(SELECT 1 FROM sdlc_task_routing_snapshots s
        JOIN sdlc_confirmations c ON c.id=s.confirmation_id AND c.task_id=s.task_id
        WHERE s.task_id=NEW.task_id AND s.payload=NEW.receipt->'task'->'routing'
        AND s.requirement_revision=(task.state->'revisions'->-1->>'revision')::bigint
        AND s.content_hash=task.state->'revisions'->-1->>'content_hash'
        AND task.root_task_id=task.task_id
        AND c.payload->>'owner_subject'=task.owner_subject
        AND NEW.receipt->'task'->>'owner_subject'=task.owner_subject) THEN
        RAISE EXCEPTION 'SDLC exact owner consent and routing required' USING ERRCODE='23514';
    END IF;
    IF TG_OP='INSERT' THEN
        IF (NEW.receipt->'task'->>'status'='Ready'
            AND NEW.receipt->'task'->>'runtime'='queued'
            AND NEW.receipt->'task'->'version'='1'::jsonb
            AND NEW.receipt->'task'->'cycle'='0'::jsonb
            AND NEW.receipt->'assignment'='null'::jsonb
            AND NEW.receipt->'lease'='null'::jsonb AND NEW.receipt->'binding'='null'::jsonb
            AND ((task.state->>'stage'='Analysis' AND NEW.receipt->'task'->>'stage'='Analysis')
                OR (task.state->>'stage'='Architecture' AND NEW.receipt->'task'->>'stage'='Architecture'
                    AND EXISTS(SELECT 1 FROM sdlc_analysis_results r WHERE r.task_id=NEW.task_id
                        AND r.result_id::text=NEW.receipt->'task'->>'previous_result_id')))) IS NOT TRUE THEN
            RAISE EXCEPTION 'SDLC delivery requires exact initial queue or retained Analysis terminal' USING ERRCODE='23514';
        END IF;
    END IF;
    IF TG_OP='UPDATE' THEN
        IF NEW.task_id<>OLD.task_id OR NEW.receipt->'task'->'routing'<>OLD.receipt->'task'->'routing' THEN
            RAISE EXCEPTION 'SDLC delivery identity and routing immutable' USING ERRCODE='23514';
        END IF;
        IF (NEW.receipt->'task'->>'version')::bigint NOT BETWEEN
            (OLD.receipt->'task'->>'version')::bigint AND (OLD.receipt->'task'->>'version')::bigint+1 THEN
            RAISE EXCEPTION 'SDLC delivery version CAS' USING ERRCODE='23514';
        END IF;
        IF NEW.receipt->'task'->'stage' IS DISTINCT FROM OLD.receipt->'task'->'stage'
            OR NEW.receipt->'task'->'cycle' IS DISTINCT FROM OLD.receipt->'task'->'cycle'
            OR (NEW.receipt->'assignment'='null'::jsonb AND OLD.receipt->'assignment'<>'null'::jsonb)
            OR (NEW.receipt->'task'->>'status'='Completed'
                AND NEW.receipt->'task'->'status' IS DISTINCT FROM OLD.receipt->'task'->'status') THEN
            IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_results r WHERE r.task_id=NEW.task_id
                AND r.result_id::text=NEW.receipt->'task'->>'previous_result_id'
                AND r.assignment_id::text=OLD.receipt->'assignment'->>'assignment_id'
                AND r.payload->'next_stage'=NEW.receipt->'task'->'stage'
                AND r.payload->'next_status'=NEW.receipt->'task'->'status'
                AND r.payload->'cycle'=NEW.receipt->'task'->'cycle') THEN
                RAISE EXCEPTION 'SDLC lifecycle transition requires accepted typed result' USING ERRCODE='23514';
            END IF;
        END IF;
        previous_lease := OLD.receipt->'lease'; next_lease := NEW.receipt->'lease';
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
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_delivery_head_gate BEFORE INSERT OR UPDATE ON sdlc_delivery_tasks
FOR EACH ROW EXECUTE FUNCTION sdlc_delivery_head_gate();
