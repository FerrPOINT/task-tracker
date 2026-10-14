-- Unregistered additive fragment. Main owns migration ordering/registration.
-- This slice persists atomic POST -> Held only. It does NOT implement ReturnToPm.
CREATE TABLE sdlc_critical_blockers (
    blocker_id uuid PRIMARY KEY CHECK(blocker_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    assignment_id uuid NOT NULL UNIQUE REFERENCES sdlc_delivery_assignments(assignment_id) ON DELETE RESTRICT,
    payload jsonb NOT NULL,
    previous_receipt jsonb NOT NULL,
    UNIQUE(blocker_id,task_id),
    CHECK ((payload->>'blocker_id'=blocker_id::text AND payload->>'task_id'=task_id::text
        AND payload->>'root_task_id'=task_id::text
        AND payload->'proposal'->'fence'->>'assignment_id'=assignment_id::text
        AND payload->'version'='1'::jsonb AND payload->>'state'='held'
        AND payload->'routing_generation'='1'::jsonb
        AND payload->>'blocker_sha256' ~ '^[0-9a-f]{64}$'
        AND payload->>'assignment_hash' ~ '^[0-9a-f]{64}$'
        AND payload->'pm_review'='null'::jsonb AND payload->'resolution'='null'::jsonb
        AND payload->'technical_terminal'='null'::jsonb
        AND payload->'proposal'->>'kind' IN ('requirements_contradiction','missing_business_decision')
        AND jsonb_typeof(payload->'proposal'->'options')='array'
        AND jsonb_array_length(payload->'proposal'->'options') BETWEEN 0 AND 8
        AND jsonb_typeof(payload->'proposal'->'evidence')='array'
        AND jsonb_array_length(payload->'proposal'->'evidence') BETWEEN 1 AND 16) IS TRUE)
);
CREATE INDEX sdlc_critical_blockers_task ON sdlc_critical_blockers(task_id);

CREATE TABLE sdlc_critical_blocker_heads (
    task_id uuid PRIMARY KEY REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    blocker_id uuid NOT NULL UNIQUE,
    FOREIGN KEY(blocker_id,task_id) REFERENCES sdlc_critical_blockers(blocker_id,task_id) ON DELETE RESTRICT
);

CREATE TABLE sdlc_critical_blocker_operations (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128
        AND idempotency_key !~ '[[:space:][:cntrl:]]'),
    payload_hash text NOT NULL CHECK(payload_hash ~ '^[0-9a-f]{64}$'),
    blocker_id uuid NOT NULL UNIQUE,
    result jsonb NOT NULL,
    event_id uuid NOT NULL UNIQUE REFERENCES sdlc_outbox(event_id) DEFERRABLE INITIALLY DEFERRED,
    PRIMARY KEY(task_id,actor_subject,idempotency_key),
    FOREIGN KEY(blocker_id,task_id) REFERENCES sdlc_critical_blockers(blocker_id,task_id) ON DELETE RESTRICT,
    CHECK ((result->>'task_id'=task_id::text AND result->>'producer_subject'=actor_subject
        AND result->>'blocker_id'=blocker_id::text AND result->>'blocker_sha256'=payload_hash
        AND result->'proposal'->>'idempotency_key'=idempotency_key
        AND result->>'state'='held' AND result->'version'='1'::jsonb AND event_id=blocker_id) IS TRUE)
);

CREATE TRIGGER sdlc_critical_blocker_immutable BEFORE UPDATE OR DELETE ON sdlc_critical_blockers
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
CREATE TRIGGER sdlc_critical_blocker_head_immutable BEFORE UPDATE OR DELETE ON sdlc_critical_blocker_heads
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
CREATE TRIGGER sdlc_critical_blocker_operation_immutable BEFORE UPDATE OR DELETE ON sdlc_critical_blocker_operations
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE FUNCTION sdlc_critical_blocker_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task_root uuid; task_state jsonb; task_owner text; task_instance text;
    task_project uuid; head jsonb; body jsonb; expected_time timestamptz;
BEGIN
    SELECT root_task_id,state,owner_subject,tracker_instance_id,project_id
        INTO task_root,task_state,task_owner,task_instance,task_project
        FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    body := NEW.payload->'proposal';
    expected_time := clock_timestamp();
    IF NEW.previous_receipt IS DISTINCT FROM head
        OR EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads WHERE task_id=NEW.task_id)
        OR (task_root=NEW.task_id AND task_state->>'flow'='single-task-v1'
            AND head->'task'->>'status'='InProgress' AND head->'task'->>'runtime'='running'
            AND head->'task'->'routing'->'contract_version'='2'::jsonb
            AND (head->'task'->>'version')::bigint BETWEEN 1 AND 9007199254740990
            AND head->'assignment'->>'assignment_id'=NEW.assignment_id::text
            AND head->'assignment'->'execution_id'=body->'fence'->'execution_id'
            AND head->'assignment'->'fencing_token'=body->'fence'->'fencing_token'
            AND head->'assignment'->'lease_id'=body->'fence'->'lease_id'
            AND head->'lease'->'lease_id'=body->'fence'->'lease_id'
            AND jsonb_typeof(body->'fence'->'expected_lease_version')='number'
            AND (body->'fence'->>'expected_lease_version')::bigint
                BETWEEN 1 AND (head->'lease'->>'version')::bigint
            AND (head->'lease'->>'expires_at')::timestamptz>expected_time
            AND (NEW.payload->>'created_at')::timestamptz<=(head->'lease'->>'expires_at')::timestamptz
            AND NEW.payload->>'tracker_instance_id'=task_instance
            AND NEW.payload->>'project_id'=task_project::text
            AND NEW.payload->>'owner_subject'=task_owner
            AND NEW.payload->'producer_subject'=head->'binding'->'machine_subject'
            AND NEW.payload->'agent_id'=head->'assignment'->'agent_id'
            AND NEW.payload->'role'=head->'assignment'->'role'
            AND NEW.payload->'assignment_hash'=head->'assignment'->'assignment_hash'
            AND NEW.payload->'chat_id'=head->'binding'->'chat_id'
            AND NEW.payload->'run_id'=head->'binding'->'run_id'
            AND NEW.payload->'routing_snapshot_id'=head->'task'->'routing'->'snapshot_id'
            AND body->'requirement_revision'=head->'task'->'routing'->'requirement_revision'
            AND body->'content_hash'=head->'task'->'routing'->'content_hash') IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC critical POST requires exact current running assignment' USING ERRCODE='23514';
    END IF;
    IF NOT EXISTS(SELECT 1 FROM sdlc_task_routing_snapshots s WHERE s.task_id=NEW.task_id
        AND s.payload=head->'task'->'routing')
        OR NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds h WHERE h.task_id=NEW.task_id
            AND h.assignment_id=NEW.assignment_id) THEN
        RAISE EXCEPTION 'SDLC critical POST requires original routing and retained capacity' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_critical_blocker_insert_gate BEFORE INSERT ON sdlc_critical_blockers
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_blocker_insert_gate();

-- A proposal/head/journal without the actual held projection cannot commit, in either order.
CREATE FUNCTION sdlc_critical_blocker_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb; expected jsonb;
BEGIN
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=NEW.task_id;
    expected := jsonb_set(NEW.previous_receipt,'{task,version}',
        to_jsonb((NEW.previous_receipt->'task'->>'version')::bigint+1));
    expected := jsonb_set(expected,'{task,waiting_reason}','"critical_business_question_held"'::jsonb);
    IF head IS DISTINCT FROM expected
        OR NOT EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads h
            WHERE h.task_id=NEW.task_id AND h.blocker_id=NEW.blocker_id)
        OR NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds h
            WHERE h.task_id=NEW.task_id AND h.assignment_id=NEW.assignment_id)
        OR NOT EXISTS(SELECT 1 FROM sdlc_critical_blocker_operations o
            JOIN sdlc_outbox e ON e.event_id=o.event_id AND e.task_id=o.task_id
            WHERE o.blocker_id=NEW.blocker_id AND o.task_id=NEW.task_id AND o.result=NEW.payload
                AND e.event_type='critical.blocker_held' AND e.payload->'result'=NEW.payload) THEN
        RAISE EXCEPTION 'SDLC critical proposal and held projection must commit atomically' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_critical_blocker_commit_gate AFTER INSERT ON sdlc_critical_blockers
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_critical_blocker_commit_gate();

-- Additive guard: existing projection/consent/lease gates still run unmodified.
CREATE FUNCTION sdlc_critical_projection_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE active_before jsonb; expected jsonb;
BEGIN
    SELECT b.previous_receipt INTO active_before FROM sdlc_critical_blocker_heads h JOIN sdlc_critical_blockers b
        ON b.blocker_id=h.blocker_id AND b.task_id=h.task_id WHERE h.task_id=NEW.task_id;
    IF NOT FOUND THEN
        IF TG_TABLE_NAME='sdlc_tasks' THEN
            IF NEW.state->'lifecycle'->>'waiting_reason'='critical_business_question_held' THEN
                RAISE EXCEPTION 'SDLC critical waiting requires a persisted blocker' USING ERRCODE='23514';
            END IF;
        END IF;
        RETURN NEW;
    END IF;
    IF TG_TABLE_NAME='sdlc_tasks' THEN
        IF OLD.state->'lifecycle'->>'waiting_reason'='critical_business_question_held' THEN
            IF NEW.state IS DISTINCT FROM OLD.state THEN
                RAISE EXCEPTION 'SDLC critical hold forbids business progression' USING ERRCODE='23514';
            END IF;
        ELSE
            expected := jsonb_set(OLD.state,'{waiting_reason}','"critical_business_question_held"'::jsonb);
            expected := jsonb_set(expected,'{lifecycle,waiting_reason}','"critical_business_question_held"'::jsonb);
            expected := jsonb_set(expected,'{lifecycle,version}',to_jsonb((OLD.state->'lifecycle'->>'version')::bigint+1));
            IF NEW.state IS DISTINCT FROM expected
                OR OLD.state->'lifecycle' IS DISTINCT FROM active_before->'task' THEN
                RAISE EXCEPTION 'SDLC critical hold changes waiting only' USING ERRCODE='23514';
            END IF;
        END IF;
    ELSE
        IF NEW.execution-'lease' IS DISTINCT FROM OLD.execution-'lease' THEN
            RAISE EXCEPTION 'SDLC critical hold retains exact assignment/run custody' USING ERRCODE='23514';
        END IF;
        -- Existing sdlc_delivery_head_gate verifies every actual lease CAS/TTL update.
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_critical_canonical_gate BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_projection_gate();
CREATE TRIGGER sdlc_critical_execution_gate BEFORE UPDATE ON sdlc_delivery_heads
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_projection_gate();

CREATE FUNCTION sdlc_critical_no_progress_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task uuid;
BEGIN
    IF TG_OP='DELETE' THEN task := OLD.task_id; ELSE task := NEW.task_id; END IF;
    PERFORM task_id FROM sdlc_tasks WHERE task_id=task FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads WHERE task_id=task) THEN
        RAISE EXCEPTION 'SDLC critical hold forbids technical progression or capacity release' USING ERRCODE='23514';
    END IF;
    IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END $$;
CREATE TRIGGER sdlc_critical_assignment_gate BEFORE INSERT ON sdlc_delivery_assignments
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_no_progress_gate();
CREATE TRIGGER sdlc_critical_result_gate BEFORE INSERT ON sdlc_delivery_results
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_no_progress_gate();
CREATE TRIGGER sdlc_critical_reconcile_gate BEFORE INSERT ON sdlc_delivery_reconciliations
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_no_progress_gate();
CREATE TRIGGER sdlc_critical_proposal_gate BEFORE INSERT ON sdlc_delivery_proposals
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_no_progress_gate();
CREATE TRIGGER sdlc_critical_release_gate BEFORE DELETE ON sdlc_delivery_holds
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_no_progress_gate();
