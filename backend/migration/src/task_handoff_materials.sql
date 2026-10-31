-- Genuine current-stage human consent, separate from execution and PM completion.
CREATE TABLE task_handoff_materials (
    handoff_id uuid PRIMARY KEY,
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    actor_id uuid NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    command_hash text NOT NULL CHECK(command_hash ~ '^[0-9a-f]{64}$'),
    previous_revision bigint NOT NULL CHECK(previous_revision BETWEEN 1 AND 9007199254740990),
    resulting_revision bigint NOT NULL CHECK(resulting_revision=previous_revision+1),
    confirmation_id uuid NOT NULL,
    requirement_revision bigint NOT NULL,
    requirements_sha256 text NOT NULL,
    receipt jsonb NOT NULL,
    previous_state jsonb NOT NULL,
    next_state jsonb NOT NULL,
    routing jsonb NOT NULL,
    source_binding jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(task_id,actor_subject,idempotency_key),
    UNIQUE(task_id,resulting_revision),
    FOREIGN KEY(task_id,confirmation_id,requirement_revision,requirements_sha256)
        REFERENCES sdlc_confirmations(task_id,id,revision,content_hash),
    CHECK((receipt->>'handoff_id'=handoff_id::text AND receipt->>'task_id'=task_id::text
        AND receipt->>'actor_subject'=actor_subject AND receipt->>'idempotency_key'=idempotency_key
        AND receipt->'resulting_revision'=to_jsonb(resulting_revision)
        AND receipt->>'confirmation_id'=confirmation_id::text
        AND receipt->'requirement_revision'=to_jsonb(requirement_revision)
        AND receipt->>'requirements_sha256'=requirements_sha256
        AND routing->>'task_id'=task_id::text AND source_binding->>'task_id'=task_id::text
        AND routing->>'confirmation_id'=confirmation_id::text
        AND source_binding->>'confirmation_id'=confirmation_id::text
        AND routing->'requirement_revision'=to_jsonb(requirement_revision)
        AND source_binding->'requirement_revision'=to_jsonb(requirement_revision)
        AND routing->>'content_hash'=requirements_sha256 AND source_binding->>'content_hash'=requirements_sha256
        AND routing->>'snapshot_id'=receipt->>'routing_snapshot_id'
        AND source_binding->>'routing_snapshot_id'=receipt->>'routing_snapshot_id'
        AND next_state->>'human_handoff_id'=handoff_id::text) IS TRUE)
);
CREATE TRIGGER task_handoff_materials_immutable BEFORE UPDATE OR DELETE ON task_handoff_materials
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE TABLE task_handoff_material_write_permits (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    transaction_id xid8 NOT NULL,
    handoff_id uuid NOT NULL REFERENCES task_handoff_materials(handoff_id) ON DELETE RESTRICT,
    PRIMARY KEY(task_id,transaction_id)
);

CREATE FUNCTION task_handoff_material_write_permitted(task uuid, next_snapshot jsonb) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT (SELECT enabled FROM task_lifecycle_control WHERE singleton)
        AND NOT task_lifecycle_execution_busy(task)
        AND EXISTS(SELECT 1 FROM task_handoff_material_write_permits p
            JOIN task_handoff_materials m ON m.handoff_id=p.handoff_id AND m.task_id=p.task_id
            JOIN sdlc_tasks t ON t.task_id=m.task_id
            JOIN sdlc_requirements r ON r.task_id=m.task_id AND r.revision=m.requirement_revision
            JOIN sdlc_confirmations c ON c.task_id=m.task_id AND c.id=m.confirmation_id
            WHERE p.task_id=task AND p.transaction_id=pg_current_xact_id()
                AND m.next_state=next_snapshot AND m.previous_state=t.state AND m.previous_revision=t.core_revision
                AND m.actor_subject=t.owner_subject
                AND EXISTS(SELECT 1 FROM issues i JOIN users u ON u.id=i.reporter_id
                    WHERE i.id=t.task_id AND u.id=m.actor_id AND u.central_sub=m.actor_subject AND u.is_active)
                AND m.receipt->>'project_id'=t.project_id::text
                AND m.receipt->'entry'->'state'->>'stage'=task_lifecycle_effective_stage(t.state)
                AND m.receipt->'entry'->'state'->>'status'=task_lifecycle_effective_status(t.state)
                AND m.receipt->'entry'->'state'->'revision'=to_jsonb(t.core_revision)
                AND m.routing->>'project_id'=t.project_id::text
                AND m.source_binding->>'project_id'=t.project_id::text
                AND m.routing->>'tracker_instance_id'=t.tracker_instance_id
                AND m.source_binding->>'tracker_instance_id'=t.tracker_instance_id
                AND EXISTS(SELECT 1 FROM sdlc_project_routing_revisions r
                    JOIN sdlc_project_routing_heads h USING(project_id)
                    WHERE r.project_id=t.project_id AND r.version=h.version
                        AND m.routing->'policy'=r.payload)
                AND EXISTS(SELECT 1 FROM sdlc_source_policy_revisions s
                    JOIN sdlc_source_policy_heads h USING(project_id)
                    JOIN tracker_namespace_bindings b ON b.resource_id=s.project_id AND b.state='active'
                    WHERE s.project_id=t.project_id AND s.source_revision=h.source_revision
                        AND m.source_binding->'source_revision'=to_jsonb(s.source_revision)
                        AND m.source_binding->>'policy_hash'=s.source_hash
                        AND m.source_binding->'namespace'=s.payload->'verification'->'namespace'
                        AND m.source_binding->'namespace'->>'registry_instance_id'=b.registry_instance_id::text
                        AND m.source_binding->'namespace'->>'namespace_id'=b.namespace_id::text)
                AND (next_snapshot-ARRAY['revisions','confirmations','confirmation_revision','human_handoff_id'])
                    =(t.state-ARRAY['revisions','confirmations','confirmation_revision','human_handoff_id'])
                AND next_snapshot->'revisions'=COALESCE(t.state->'revisions','[]'::jsonb)||jsonb_build_array(r.payload)
                AND next_snapshot->'confirmations'=COALESCE(t.state->'confirmations','[]'::jsonb)||jsonb_build_array(c.payload)
                AND next_snapshot->'confirmation_revision'=to_jsonb(m.requirement_revision)
                AND c.payload->>'owner_subject'=m.actor_subject
                AND r.payload->>'author_subject'=m.actor_subject
                AND c.payload->>'stage'=t.state->>'stage')
$$;

-- Add a narrow, ledger-backed consent branch. Retain every existing transition,
-- identity, execution-fencing and PM-completion guard for all other writes.
DO $$ DECLARE definition text; anchor text; name text; BEGIN
    definition:=pg_get_functiondef('task_lifecycle_state_guard()'::regprocedure);
    anchor:='    IF NEW.manual_control IS DISTINCT FROM OLD.manual_control';
    IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'handoff_state_guard_anchor_missing'; END IF;
    definition:=replace(definition,anchor,$branch$
    IF task_handoff_material_write_permitted(OLD.task_id,NEW.state) THEN
        IF NOT NEW.manual_control
            OR NEW.pm_execution_id IS DISTINCT FROM OLD.pm_execution_id
            OR NEW.pm_owner_version IS DISTINCT FROM OLD.pm_owner_version
            OR NEW.pm_admission_state IS DISTINCT FROM OLD.pm_admission_state THEN
            RAISE EXCEPTION 'task_handoff_materials_cannot_allocate_execution' USING ERRCODE='23514';
        END IF;
        IF NOT OLD.manual_control THEN
            INSERT INTO task_lifecycle_custody_history(task_id,core_revision,previous_state,previous_execution)
            VALUES(OLD.task_id,OLD.core_revision,OLD.state,
                (SELECT execution FROM sdlc_delivery_heads WHERE task_id=OLD.task_id));
        END IF;
        NEW.core_revision:=OLD.core_revision+1;
        RETURN NEW;
    END IF;
$branch$||anchor);
    EXECUTE definition;
    FOREACH name IN ARRAY ARRAY['sdlc_pm_reservation_gate','sdlc_delivery_canonical_gate',
        'sdlc_configured_cli_completion_transition'] LOOP
        definition:=pg_get_functiondef(to_regprocedure(name||'()'));
        anchor:='task_lifecycle_manual_write_permitted(OLD.task_id,NEW.state)';
        IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'handoff_execution_guard_anchor_missing:%',name; END IF;
        definition:=replace(definition,anchor,'('||anchor||' OR task_handoff_material_write_permitted(OLD.task_id,NEW.state))');
        EXECUTE definition;
    END LOOP;
END $$;
DROP TRIGGER sdlc_analysis_gate ON sdlc_tasks;
CREATE TRIGGER sdlc_analysis_gate BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW WHEN (NOT task_lifecycle_manual_write_permitted(OLD.task_id,NEW.state)
    AND NOT task_handoff_material_write_permitted(OLD.task_id,NEW.state))
EXECUTE FUNCTION sdlc_analysis_gate();
