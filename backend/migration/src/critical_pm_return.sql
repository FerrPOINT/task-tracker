-- Unregistered additive fragment. Main owns migration ordering/registration.
-- Parent includes after single_task_lifecycle.sql in the ONE unreleased 096 packet.
-- Atomic hold and scheduler terminal return only: no PM reservation or routing generation.
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
        AND idempotency_key !~ '[[:space:][:cntrl:]]' AND idempotency_key NOT IN ('.','..')),
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

-- Append-only terminal handoff journal AND actual old-head archive. Original Held v1 stays intact.
CREATE TABLE sdlc_critical_pm_returns (
    return_id uuid PRIMARY KEY CHECK(return_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    blocker_id uuid NOT NULL UNIQUE,
    assignment_id uuid NOT NULL UNIQUE REFERENCES sdlc_delivery_assignments(assignment_id) ON DELETE RESTRICT,
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128
        AND idempotency_key !~ '[[:space:][:cntrl:]]' AND idempotency_key NOT IN ('.','..')),
    request_sha256 text NOT NULL CHECK(request_sha256 ~ '^[0-9a-f]{64}$'),
    command jsonb NOT NULL, before_receipt jsonb NOT NULL,
    after_receipt jsonb NOT NULL, after_blocker jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(task_id,actor_subject,idempotency_key),
    FOREIGN KEY(blocker_id,task_id) REFERENCES sdlc_critical_blockers(blocker_id,task_id) ON DELETE RESTRICT,
    FOREIGN KEY(return_id) REFERENCES sdlc_outbox(event_id) DEFERRABLE INITIALLY DEFERRED,
    CHECK ((command->'review'->>'blocker_id'=blocker_id::text
        AND command->'review'->>'idempotency_key'=idempotency_key
        AND before_receipt->'assignment'->>'assignment_id'=assignment_id::text
        AND before_receipt->'lease'->>'holder_subject'=actor_subject
        AND command->'terminal'->>'state' IN ('succeeded','failed','cancelled')
        AND command->'terminal'->'receipt'->>'sha256' ~ '^[0-9a-f]{64}$'
        AND octet_length(btrim(command->'terminal'->'receipt'->>'reference')) BETWEEN 1 AND 512
        AND command->'terminal'->'receipt'->>'reference' !~ '[[:cntrl:]]'
        AND after_blocker->>'blocker_id'=blocker_id::text AND after_blocker->>'task_id'=task_id::text
        AND after_blocker->>'state'='awaiting_pm_review' AND after_blocker->'version'='2'::jsonb
        AND after_blocker->'technical_terminal'=command->'terminal') IS TRUE)
);
CREATE TRIGGER sdlc_critical_pm_return_immutable BEFORE UPDATE OR DELETE ON sdlc_critical_pm_returns
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

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

CREATE FUNCTION sdlc_critical_pm_return_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb; blocker jsonb; expected jsonb; expected_task jsonb; review jsonb;
BEGIN
    PERFORM task_id FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT b.payload INTO blocker FROM sdlc_critical_blockers b JOIN sdlc_critical_blocker_heads h
        ON h.task_id=b.task_id AND h.blocker_id=b.blocker_id
        WHERE b.task_id=NEW.task_id AND b.blocker_id=NEW.blocker_id;
    review := NEW.command->'review';
    IF NEW.before_receipt IS DISTINCT FROM head
        OR (blocker->>'state'='held' AND blocker->'version'='1'::jsonb
            AND review->'expected_blocker_version'=blocker->'version'
            AND review->'blocker_sha256'=blocker->'blocker_sha256'
            AND review->'expected_task_version'=head->'task'->'version'
            AND (head->'task'->>'version')::bigint BETWEEN 1 AND 9007199254740990
            AND head->'task'->>'waiting_reason'='critical_business_question_held'
            AND head->'task'->>'status'='InProgress' AND head->'task'->>'runtime' IN ('running','unknown')
            AND head->'assignment'->>'assignment_id'=NEW.assignment_id::text
            AND review->'fence'->>'assignment_id'=NEW.assignment_id::text
            AND review->'fence'->'execution_id'=head->'assignment'->'execution_id'
            AND review->'fence'->'fencing_token'=head->'assignment'->'fencing_token'
            AND review->'fence'->'lease_id'=head->'assignment'->'lease_id'
            AND review->'fence'->'lease_id'=head->'lease'->'lease_id'
            AND review->'fence'->'expected_lease_version'=head->'lease'->'version'
            AND (head->'lease'->>'version')::bigint BETWEEN 1 AND 9007199254740991
            AND head->'lease'->>'holder_subject'=NEW.actor_subject
            AND NEW.command->'terminal'->'assignment_hash'=head->'assignment'->'assignment_hash'
            AND NEW.command->'terminal'->'chat_id'=head->'binding'->'chat_id'
            AND NEW.command->'terminal'->'run_id'=head->'binding'->'run_id'
            AND blocker->'assignment_hash'=head->'assignment'->'assignment_hash'
            AND blocker->'chat_id'=head->'binding'->'chat_id' AND blocker->'run_id'=head->'binding'->'run_id'
            AND blocker->'proposal'->'fence'->'assignment_id'=head->'assignment'->'assignment_id'
            AND blocker->'proposal'->'fence'->'execution_id'=head->'assignment'->'execution_id'
            AND blocker->'proposal'->'fence'->'fencing_token'=head->'assignment'->'fencing_token'
            AND blocker->'proposal'->'fence'->'lease_id'=head->'assignment'->'lease_id'
            AND blocker->'routing_snapshot_id'=head->'task'->'routing'->'snapshot_id') IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC critical return requires current held terminal assignment and exact scheduler CAS' USING ERRCODE='23514';
    END IF;
    -- Expiry/stop ACK is not terminal proof. The configured trusted scheduler supplies actual readback;
    -- application authorization and the closed native terminal DTO remain mandatory.
    expected := jsonb_set(blocker,'{version}','2'::jsonb);
    expected := jsonb_set(expected,'{state}','"awaiting_pm_review"'::jsonb);
    expected := jsonb_set(expected,'{technical_terminal}',NEW.command->'terminal');
    IF NEW.after_blocker IS DISTINCT FROM expected THEN
        RAISE EXCEPTION 'SDLC critical return cannot rewrite original blocker' USING ERRCODE='23514';
    END IF;
    expected_task := jsonb_set(head->'task','{version}',to_jsonb((head->'task'->>'version')::bigint+1));
    expected_task := jsonb_set(expected_task,'{stage}','"Draft"'::jsonb);
    expected_task := jsonb_set(expected_task,'{status}','"Ready"'::jsonb);
    expected_task := jsonb_set(expected_task,'{runtime}','"terminal"'::jsonb);
    expected_task := jsonb_set(expected_task,'{waiting_reason}','"critical_pm_review_queued"'::jsonb);
    expected_task := jsonb_set(expected_task,'{previous_result_id}','null'::jsonb)-'resume_chat_id';
    expected := jsonb_set(head,'{task}',expected_task);
    expected := jsonb_set(expected,'{assignment}','null'::jsonb);
    expected := jsonb_set(expected,'{lease}','null'::jsonb);
    expected := jsonb_set(expected,'{binding}','null'::jsonb);
    expected := jsonb_set(expected,'{result}','null'::jsonb);
    IF NEW.after_receipt IS DISTINCT FROM expected
        OR NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds h
            WHERE h.task_id=NEW.task_id AND h.assignment_id=NEW.assignment_id)
        OR NOT EXISTS(SELECT 1 FROM sdlc_task_routing_snapshots s
            WHERE s.task_id=NEW.task_id AND s.payload=head->'task'->'routing') THEN
        RAISE EXCEPTION 'SDLC critical return requires exact archived head, routing and retained capacity' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_critical_pm_return_insert_gate BEFORE INSERT ON sdlc_critical_pm_returns
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_return_insert_gate();

CREATE FUNCTION sdlc_critical_pm_return_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_tasks d WHERE d.task_id=NEW.task_id AND d.receipt=NEW.after_receipt)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_holds h WHERE h.task_id=NEW.task_id OR h.assignment_id=NEW.assignment_id)
        OR NOT EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads h JOIN sdlc_critical_blockers b
            ON b.task_id=h.task_id AND b.blocker_id=h.blocker_id
            WHERE h.task_id=NEW.task_id AND h.blocker_id=NEW.blocker_id
                AND b.payload=(NEW.after_blocker-'technical_terminal'-'state'-'version'
                    || jsonb_build_object('technical_terminal',NULL,'state','held','version',1)))
        OR NOT EXISTS(SELECT 1 FROM sdlc_outbox e WHERE e.event_id=NEW.return_id AND e.task_id=NEW.task_id
            AND e.event_type='critical.returned_to_pm' AND e.payload->'result'=NEW.after_blocker
            AND e.payload->'lifecycle'=NEW.after_receipt) THEN
        RAISE EXCEPTION 'SDLC critical return archive, release, canonical state and event must commit atomically' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_critical_pm_return_commit_gate AFTER INSERT ON sdlc_critical_pm_returns
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_return_commit_gate();

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

-- Additive guard: existing projection/consent/lease gates still run (narrow journal exceptions below).
CREATE FUNCTION sdlc_critical_projection_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE active_before jsonb; expected jsonb; returned_before jsonb; returned_after jsonb;
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
        SELECT r.before_receipt,r.after_receipt INTO returned_before,returned_after
            FROM sdlc_critical_pm_returns r JOIN sdlc_critical_blocker_heads h
                ON h.task_id=r.task_id AND h.blocker_id=r.blocker_id WHERE r.task_id=NEW.task_id;
        IF FOUND AND OLD.state->'lifecycle'=returned_before->'task'
            AND NEW.state->'lifecycle'=returned_after->'task' THEN
            expected := jsonb_set(OLD.state,'{lifecycle}',returned_after->'task');
            expected := jsonb_set(expected,'{stage}',returned_after->'task'->'stage');
            expected := jsonb_set(expected,'{status}',returned_after->'task'->'status');
            expected := jsonb_set(expected,'{runtime_state}',returned_after->'task'->'runtime');
            expected := jsonb_set(expected,'{waiting_reason}',returned_after->'task'->'waiting_reason');
            IF NEW.state IS DISTINCT FROM expected THEN
                RAISE EXCEPTION 'SDLC critical return cannot change unrelated state' USING ERRCODE='23514';
            END IF;
            RETURN NEW;
        END IF;
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
        IF EXISTS(SELECT 1 FROM sdlc_critical_pm_returns r
            WHERE r.task_id=NEW.task_id AND r.before_receipt-'task'=OLD.execution
                AND r.after_receipt-'task'=NEW.execution) THEN RETURN NEW; END IF;
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
        IF TG_OP='DELETE' AND TG_TABLE_NAME='sdlc_delivery_holds' THEN
            IF EXISTS(SELECT 1 FROM sdlc_critical_pm_returns r
                JOIN sdlc_delivery_tasks d ON d.task_id=r.task_id AND d.receipt=r.before_receipt
                WHERE r.task_id=OLD.task_id AND r.assignment_id=OLD.assignment_id) THEN RETURN OLD; END IF;
        END IF;
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

-- Preserve existing result/reconcile paths. Only this immutable terminal-return journal adds a path;
-- canonical permits, original owner consent and lease CAS below are not bypassed.
CREATE OR REPLACE FUNCTION sdlc_delivery_hold_release_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_results WHERE assignment_id=OLD.assignment_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_reconciliations WHERE assignment_id=OLD.assignment_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_critical_pm_returns r JOIN sdlc_delivery_tasks d
            ON d.task_id=r.task_id AND d.receipt=r.before_receipt
            WHERE r.task_id=OLD.task_id AND r.assignment_id=OLD.assignment_id) THEN
        RAISE EXCEPTION 'SDLC terminal receipt required before capacity release' USING ERRCODE='23514';
    END IF;
    RETURN OLD;
END $$;

CREATE OR REPLACE FUNCTION sdlc_delivery_head_gate() RETURNS trigger LANGUAGE plpgsql AS $$
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
                    AND r.assignment_id::text=OLD.execution->'assignment'->>'assignment_id')
                AND NOT EXISTS(SELECT 1 FROM sdlc_critical_pm_returns r WHERE r.task_id=NEW.task_id
                    AND r.assignment_id::text=OLD.execution->'assignment'->>'assignment_id'
                    AND r.before_receipt-'task'=OLD.execution AND r.after_receipt-'task'=NEW.execution
                    AND r.after_receipt->'task'=task.state->'lifecycle') THEN
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

CREATE OR REPLACE FUNCTION sdlc_delivery_canonical_gate() RETURNS trigger LANGUAGE plpgsql AS $$
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
        IF EXISTS(SELECT 1 FROM sdlc_critical_pm_returns r WHERE r.task_id=OLD.task_id
            AND r.before_receipt->'task'=old_task AND r.after_receipt->'task'=next_task) THEN
            RETURN NEW;
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
