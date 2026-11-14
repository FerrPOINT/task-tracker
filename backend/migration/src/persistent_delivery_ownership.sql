-- One current technical ownership projection. Historical PM/Analysis receipts
-- and every assignment/operation/result row retain their original bytes.
CREATE OR REPLACE FUNCTION sdlc_delivery_head_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task sdlc_tasks%ROWTYPE; previous_lease jsonb; next_lease jsonb;
BEGIN
    SELECT * INTO task FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p WHERE p.task_id=NEW.task_id
        AND p.transaction_id=pg_current_xact_id() AND p.next_state=task.state AND p.next_execution=NEW.execution) THEN
        RAISE EXCEPTION 'SDLC execution cursor requires canonical transaction permit' USING ERRCODE='23514';
    END IF;
    IF NEW.execution->'assignment'<>'null'::jsonb
        AND (jsonb_typeof(NEW.execution->'lease')='object'
            AND NEW.execution#>'{lease,expires_at}'='null'::jsonb) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC active assignment ownership cannot expire' USING ERRCODE='23514';
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
            IF ((next_lease-'expires_at')=(previous_lease-'expires_at')
                AND next_lease->'expires_at'='null'::jsonb) IS NOT TRUE THEN
                RAISE EXCEPTION 'SDLC assignment ownership identity is immutable' USING ERRCODE='23514';
            END IF;
        END IF;
    END IF;
    DELETE FROM sdlc_delivery_projection_permits WHERE task_id=NEW.task_id AND transaction_id=pg_current_xact_id();
    RETURN NEW;
END $$;

-- Remove only assignment-clock predicates from the installed guard owners.
-- Exact anchors refuse an unknown predecessor instead of weakening a guard.
DO $patch$
DECLARE name text; source text; anchor text;
BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_delivery_run_gate()', 'sdlc_delivery_proposal_gate()',
        'sdlc_delivery_result_gate()', 'sdlc_agent_status_projection(uuid,jsonb,jsonb)'] LOOP
        source:=replace(pg_get_functiondef(to_regprocedure(name)),E'\r\n',E'\n');
        anchor:=$anchor$AND (head->'lease'->>'expires_at')::timestamptz>clock_timestamp()$anchor$;
        IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
            RAISE EXCEPTION 'persistent ownership guard anchor changed: %',name;
        END IF;
        EXECUTE replace(source,anchor,'');
    END LOOP;
    source:=replace(pg_get_functiondef('sdlc_critical_blocker_insert_gate()'::regprocedure),E'\r\n',E'\n');
    FOREACH anchor IN ARRAY ARRAY[
        $anchor$AND (head->'lease'->>'expires_at')::timestamptz>expected_time$anchor$,
        $anchor$AND (NEW.payload->>'created_at')::timestamptz<=(head->'lease'->>'expires_at')::timestamptz$anchor$
    ] LOOP
        IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
            RAISE EXCEPTION 'persistent ownership critical guard anchor changed';
        END IF;
        source:=replace(source,anchor,'');
    END LOOP;
    EXECUTE source;
END $patch$;

CREATE OR REPLACE FUNCTION sdlc_delivery_reconciliation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb; observation jsonb;
BEGIN
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    IF NEW.payload ? 'never_submitted' OR NEW.payload#>>'{recovery,disposition}'='retry_unsubmitted' THEN
        RAISE EXCEPTION 'SDLC assignment persists until result or confirmed terminal reconciliation' USING ERRCODE='23514';
    END IF;
    observation:=NEW.payload->'terminal';
    IF (head#>>'{assignment,assignment_id}'=NEW.assignment_id::text
        AND head#>'{assignment,execution_id}'=NEW.payload#>'{fence,execution_id}'
        AND head#>'{assignment,fencing_token}'=NEW.payload#>'{fence,fencing_token}'
        AND head#>'{assignment,lease_id}'=NEW.payload#>'{fence,lease_id}'
        AND (NEW.payload#>>'{fence,expected_lease_version}')::bigint BETWEEN 1 AND (head#>>'{lease,version}')::bigint
        AND head#>'{assignment,assignment_hash}'=observation->'assignment_hash') IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC reconciliation immutable assignment mismatch' USING ERRCODE='23514';
    END IF;
    IF (head->'binding'='null'::jsonb OR
        (head#>'{binding,run_id}'=observation->'run_id' AND head#>'{binding,chat_id}'=observation->'chat_id')) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC reconciliation requires exact actual terminal run' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

-- Adopt the existing active head without changing its assignment, holder,
-- fencing token, version, business state, operation keys or historical receipts.
INSERT INTO sdlc_delivery_projection_permits(task_id,transaction_id,next_state,next_execution)
SELECT h.task_id,pg_current_xact_id(),t.state,jsonb_set(h.execution,'{lease,expires_at}','null'::jsonb)
FROM sdlc_delivery_heads h JOIN sdlc_tasks t USING(task_id)
WHERE h.execution->'assignment'<>'null'::jsonb AND jsonb_typeof(h.execution->'lease')='object'
    AND h.execution#>'{lease,expires_at}' IS DISTINCT FROM 'null'::jsonb;
UPDATE sdlc_delivery_heads h SET execution=p.next_execution
FROM sdlc_delivery_projection_permits p
WHERE p.task_id=h.task_id AND p.transaction_id=pg_current_xact_id();
