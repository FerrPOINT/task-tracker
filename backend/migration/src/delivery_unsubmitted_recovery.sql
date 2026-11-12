-- Extend the existing reconciliation history; keep all native terminal rows unchanged.
ALTER TABLE sdlc_delivery_reconciliations DROP CONSTRAINT sdlc_delivery_reconciliations_check;
ALTER TABLE sdlc_delivery_reconciliations ADD CONSTRAINT sdlc_delivery_reconciliations_check CHECK ((
    payload->>'operation'='reconcile'
    AND payload#>>'{fence,assignment_id}'=assignment_id::text
    AND (
        (NOT (payload ? 'never_submitted')
            AND payload#>>'{recovery,disposition}' IN ('stop','retry_transient')
            AND payload#>>'{terminal,state}' IN ('succeeded','failed','cancelled')
            AND payload#>>'{terminal,receipt,sha256}' ~ '^[0-9a-f]{64}$')
        OR (NOT (payload ? 'terminal') AND jsonb_typeof(payload->'never_submitted')='object'
            AND payload#>>'{recovery,disposition}'='retry_unsubmitted'
            AND payload#>>'{never_submitted,assignment_hash}' ~ '^[0-9a-f]{64}$'
            AND payload#>>'{never_submitted,reservation_payload_sha256}' ~ '^[0-9a-f]{64}$'
            AND octet_length(payload#>>'{never_submitted,reservation_key}') BETWEEN 1 AND 128)
    )
) IS TRUE);

CREATE OR REPLACE FUNCTION sdlc_delivery_reconciliation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE head jsonb; observation jsonb;
BEGIN
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    observation:=CASE WHEN NEW.payload ? 'never_submitted'
        THEN NEW.payload->'never_submitted' ELSE NEW.payload->'terminal' END;
    IF (head#>>'{assignment,assignment_id}'=NEW.assignment_id::text
        AND head#>'{assignment,execution_id}'=NEW.payload#>'{fence,execution_id}'
        AND head#>'{assignment,fencing_token}'=NEW.payload#>'{fence,fencing_token}'
        AND head#>'{assignment,lease_id}'=NEW.payload#>'{fence,lease_id}'
        AND (NEW.payload#>>'{fence,expected_lease_version}')::bigint BETWEEN 1 AND (head#>>'{lease,version}')::bigint
        AND head#>'{assignment,assignment_hash}'=observation->'assignment_hash') IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC reconciliation immutable assignment mismatch' USING ERRCODE='23514';
    END IF;
    IF NEW.payload ? 'never_submitted' THEN
        IF (head->'binding'='null'::jsonb AND head#>>'{task,status}'='Ready'
            AND head#>>'{task,waiting_reason}'='awaiting_run_binding'
            AND head#>>'{task,runtime}' IN ('reserved','unknown')
            AND (head#>>'{lease,expires_at}')::timestamptz<=clock_timestamp()
            AND NEW.payload#>>'{recovery,disposition}'='retry_unsubmitted'
            AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_run_bindings b WHERE b.assignment_id=NEW.assignment_id)
            AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_proposals p WHERE p.assignment_id=NEW.assignment_id)
            AND EXISTS(SELECT 1 FROM sdlc_delivery_operations o
                JOIN sdlc_outbox e ON e.event_id=o.event_id AND e.task_id=o.task_id
                WHERE o.task_id=NEW.task_id AND e.event_type='delivery.assignment_reserved'
                AND o.actor_subject=head#>>'{lease,holder_subject}'
                AND o.idempotency_key=observation->>'reservation_key'
                AND o.payload_hash=observation->>'reservation_payload_sha256'
                AND o.result->'assignment'=head->'assignment'
                AND o.result#>>'{task,runtime}'='reserved')) IS NOT TRUE THEN
            RAISE EXCEPTION 'SDLC never-submitted reconciliation requires expired unbound original reservation' USING ERRCODE='23514';
        END IF;
    ELSIF (head->'binding'='null'::jsonb OR
        (head#>'{binding,run_id}'=observation->'run_id' AND head#>'{binding,chat_id}'=observation->'chat_id')) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC reconciliation requires exact actual terminal run' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
