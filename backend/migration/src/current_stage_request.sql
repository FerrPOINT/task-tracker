-- Request ownership is distinct from a running process. Reuse the immutable
-- human ledger for durable scheduler discovery; no second business-stage owner.
ALTER TABLE task_handoff_materials ADD COLUMN execution_requested boolean NOT NULL DEFAULT false;
ALTER TABLE task_handoff_materials ADD COLUMN queue_observations jsonb;
ALTER TABLE task_handoff_materials ADD CONSTRAINT task_handoff_observation_presence
    CHECK(execution_requested=(queue_observations IS NOT NULL));

CREATE FUNCTION task_handoff_queue_observation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE observation jsonb; deadline timestamptz;
BEGIN
    IF NOT NEW.execution_requested THEN RETURN NULL; END IF;
    observation:=NEW.queue_observations;
    deadline:=least((observation->>'fleet_observed_at')::timestamptz+interval '5 seconds',
        (observation->>'source_observed_at')::timestamptz+interval '30 seconds');
    IF (observation->'schema_version'='1'::jsonb
        AND observation->>'issue_materials_sha256'=NEW.receipt->>'issue_materials_sha256'
        AND observation->>'source_repository_id'=NEW.source_binding->>'repository_id'
        AND observation->'source_namespace'=NEW.source_binding->'namespace'
        AND observation->>'fleet_selection_sha256' ~ '^[0-9a-f]{64}$'
        AND (observation->>'fleet_observation_ref')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
        AND (observation->>'valid_until')::timestamptz=deadline
        AND deadline>clock_timestamp()
        AND (observation->>'fleet_observed_at')::timestamptz<=clock_timestamp()+interval '2 seconds'
        AND (observation->>'source_observed_at')::timestamptz<=clock_timestamp()) IS NOT TRUE THEN
        RAISE EXCEPTION 'task_handoff_observations_expired_or_changed' USING ERRCODE='23514';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER task_handoff_queue_observation_guard AFTER INSERT ON task_handoff_materials
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION task_handoff_queue_observation_guard();

-- Keep the existing runtime/terminal predicate verbatim, and extend the original
-- function OID so cached callers cannot miss ownership of a newly queued request.
DO $$ DECLARE body text; BEGIN
    SELECT prosrc INTO STRICT body FROM pg_proc WHERE oid='task_lifecycle_execution_busy(uuid)'::regprocedure;
    EXECUTE format('CREATE FUNCTION task_lifecycle_runtime_busy(task uuid) RETURNS boolean LANGUAGE sql STABLE AS %L',body);
END $$;

ALTER TABLE task_handoff_materials ADD CONSTRAINT task_handoff_task_identity UNIQUE(task_id,handoff_id);
CREATE TABLE task_handoff_cancellations (
    task_id uuid NOT NULL,
    handoff_id uuid PRIMARY KEY,
    actor_id uuid NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    actor_subject text NOT NULL,
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    command_hash text NOT NULL CHECK(command_hash ~ '^[0-9a-f]{64}$'),
    previous_revision bigint NOT NULL CHECK(previous_revision BETWEEN 1 AND 9007199254740990),
    resulting_revision bigint NOT NULL CHECK(resulting_revision=previous_revision+1),
    receipt jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(task_id,actor_subject,idempotency_key),
    FOREIGN KEY(task_id,handoff_id) REFERENCES task_handoff_materials(task_id,handoff_id) ON DELETE RESTRICT,
    CHECK((receipt->>'task_id'=task_id::text AND receipt->>'handoff_id'=handoff_id::text
        AND receipt->>'actor_subject'=actor_subject AND receipt->'resulting_revision'=to_jsonb(resulting_revision)) IS TRUE)
);
CREATE TRIGGER task_handoff_cancellations_immutable BEFORE UPDATE OR DELETE ON task_handoff_cancellations
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
CREATE TABLE task_handoff_cancel_write_permits (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    transaction_id xid8 NOT NULL,
    handoff_id uuid NOT NULL REFERENCES task_handoff_cancellations(handoff_id) ON DELETE RESTRICT,
    next_state jsonb NOT NULL,
    PRIMARY KEY(task_id,transaction_id)
);

CREATE OR REPLACE FUNCTION task_lifecycle_execution_busy(task uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT task_lifecycle_runtime_busy(task) OR EXISTS(SELECT 1 FROM sdlc_tasks t
        JOIN task_handoff_materials m ON m.task_id=t.task_id
            AND m.handoff_id::text=t.state->>'human_handoff_id'
        WHERE t.task_id=task AND t.manual_control AND m.execution_requested
            AND NOT EXISTS(SELECT 1 FROM task_handoff_cancellations c WHERE c.handoff_id=m.handoff_id))
$$;

CREATE FUNCTION task_handoff_cancel_write_permitted(task uuid,next_snapshot jsonb) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT NOT task_lifecycle_runtime_busy(task) AND EXISTS(
        SELECT 1 FROM task_handoff_cancel_write_permits p
        JOIN task_handoff_cancellations c ON c.task_id=p.task_id AND c.handoff_id=p.handoff_id
        JOIN task_handoff_materials m ON m.handoff_id=c.handoff_id AND m.task_id=c.task_id
        JOIN sdlc_tasks t ON t.task_id=p.task_id AND t.manual_control
        JOIN issues i ON i.id=t.task_id JOIN users u ON u.id=i.reporter_id AND u.is_active
        WHERE p.task_id=task AND p.transaction_id=pg_current_xact_id() AND p.next_state=next_snapshot
            AND t.state->>'human_handoff_id'=c.handoff_id::text AND m.execution_requested
            AND c.previous_revision=t.core_revision
            AND c.actor_subject=t.owner_subject AND c.actor_subject=m.actor_subject
            AND u.id=c.actor_id AND u.central_sub=c.actor_subject
            AND NOT next_snapshot ? 'human_handoff_id'
            AND next_snapshot=(t.state-'human_handoff_id'))
$$;

-- The common identity/core-revision branch and old execution bodies remain in
-- force. Cancellation removes only the exact queued pointer, with its own ledger.
DO $$ DECLARE name text; definition text; anchor text := 'task_handoff_material_write_permitted(OLD.task_id,NEW.state)'; BEGIN
    FOREACH name IN ARRAY ARRAY['task_lifecycle_state_guard','sdlc_pm_reservation_gate',
        'sdlc_delivery_canonical_gate','sdlc_configured_cli_completion_transition','sdlc_draft_flow_gate'] LOOP
        definition:=pg_get_functiondef(to_regprocedure(name||'()'));
        IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'handoff_cancel_guard_anchor_missing:%',name; END IF;
        EXECUTE replace(definition,anchor,'('||anchor||' OR task_handoff_cancel_write_permitted(OLD.task_id,NEW.state))');
    END LOOP;
END $$;
DROP TRIGGER sdlc_analysis_gate ON sdlc_tasks;
CREATE TRIGGER sdlc_analysis_gate BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW WHEN (NOT task_lifecycle_manual_write_permitted(OLD.task_id,NEW.state)
    AND NOT task_handoff_material_write_permitted(OLD.task_id,NEW.state)
    AND NOT task_handoff_cancel_write_permitted(OLD.task_id,NEW.state))
EXECUTE FUNCTION sdlc_analysis_gate();
