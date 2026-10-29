-- Historical operation rows stay byte-for-byte unchanged. Only the explicit
-- owner start records its closed command and correlates it with existing permits.
ALTER TABLE sdlc_delivery_operations ADD COLUMN command jsonb,
    ADD COLUMN transaction_id xid8;
ALTER TABLE sdlc_delivery_operations ADD CONSTRAINT sdlc_delivery_start_command CHECK (
    ((command IS NULL AND transaction_id IS NULL) OR
     (command IS NOT NULL AND transaction_id IS NOT NULL
      AND command->>'operation'='start_analysis'
      AND jsonb_typeof(command->'idempotency_key')='string'
      AND command-ARRAY['operation','pm_completion_id','idempotency_key']='{}'::jsonb
      AND command->>'pm_completion_id' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
      AND command->>'pm_completion_id'<>'00000000-0000-0000-0000-000000000000'
      AND command->>'idempotency_key'=idempotency_key AND event_id IS NOT NULL)) IS TRUE
);
CREATE UNIQUE INDEX sdlc_delivery_start_once ON sdlc_delivery_operations(task_id) WHERE command IS NOT NULL;

CREATE FUNCTION sdlc_configured_start_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks; c sdlc_configured_pm_completions; r sdlc_task_routing_snapshots; expected jsonb;
BEGIN
    IF NEW.command IS NULL THEN RETURN NEW; END IF;
    SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT * INTO c FROM sdlc_configured_pm_completions
        WHERE completion_id=(NEW.command->>'pm_completion_id')::uuid AND task_id=t.task_id;
    SELECT * INTO r FROM sdlc_task_routing_snapshots WHERE task_id=t.task_id AND confirmation_id=c.confirmation_id;
    IF NOT ((NEW.transaction_id=pg_current_xact_id()
        AND NEW.actor_subject=t.owner_subject AND t.root_task_id=t.task_id
        AND t.state->>'interaction_protocol'='configured-cli-v1' AND t.state->>'flow'='single-task-v1'
        AND t.state->>'stage'='Backlog' AND t.state->>'status'='Ready' AND NOT t.state ? 'lifecycle'
        AND t.state->>'owner_subject'=t.owner_subject
        AND EXISTS(SELECT 1 FROM issues i JOIN users u ON u.id=i.reporter_id
            WHERE i.id=t.task_id AND i.project_id=t.project_id AND i.deleted_at IS NULL
            AND u.is_active AND u.central_sub=t.owner_subject)
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=t.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=t.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=t.task_id)
        AND sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)
        AND c.execution_id=t.pm_execution_id AND c.result->'owner_version'=to_jsonb(t.pm_owner_version)
        AND c.result->>'owner_subject'=t.owner_subject AND c.result->>'stage'='Backlog' AND c.result->>'status'='Ready'
        AND t.state->'confirmation_revision'=to_jsonb(c.requirement_revision)
        AND r.confirmation_id=c.confirmation_id AND r.requirement_revision=c.requirement_revision AND r.content_hash=c.content_hash
        AND r.payload->'contract_version'='2'::jsonb
        AND EXISTS(SELECT 1 FROM sdlc_task_source_bindings b WHERE b.task_id=t.task_id
            AND b.project_id=t.project_id AND b.confirmation_id=c.confirmation_id
            AND b.routing_snapshot_id=r.snapshot_id AND b.requirement_revision=c.requirement_revision AND b.content_hash=c.content_hash)
        AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents WHERE task_id=t.task_id)) IS TRUE) THEN
        RAISE EXCEPTION 'SDLC explicit start requires current owner/completion/consent/source' USING ERRCODE='23514';
    END IF;
    expected:=jsonb_build_object('task',jsonb_build_object('task_id',t.task_id,'owner_subject',t.owner_subject,
        'version',1,'stage','Analysis','status','Ready','cycle',0,'transient_retries',0,
        'waiting_reason','queued_for_assignment','runtime','queued','routing',r.payload,'previous_result_id',NULL),
        'assignment',NULL,'lease',NULL,'binding',NULL,'result',NULL);
    IF NEW.result IS DISTINCT FROM expected THEN
        RAISE EXCEPTION 'SDLC explicit start must queue exact initial lifecycle only' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_start_insert_gate BEFORE INSERT ON sdlc_delivery_operations
FOR EACH ROW EXECUTE FUNCTION sdlc_configured_start_insert_gate();

CREATE FUNCTION sdlc_configured_start_exact(task uuid,previous_state jsonb,next_state jsonb) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_delivery_operations o
        JOIN sdlc_delivery_projection_permits p ON p.task_id=o.task_id AND p.transaction_id=o.transaction_id
        WHERE o.task_id=$1 AND o.command->>'operation'='start_analysis' AND o.transaction_id=pg_current_xact_id()
        AND p.next_state=$3 AND p.next_execution=o.result-'task'
        AND $2->>'stage'='Backlog' AND $2->>'status'='Ready' AND NOT $2 ? 'lifecycle'
        AND $2->>'interaction_protocol'='configured-cli-v1' AND $2->>'owner_subject'=o.actor_subject
        AND $3->>'stage'='Analysis' AND $3->>'status'='Ready'
        AND $3->>'runtime_state'='queued' AND $3->>'waiting_reason'='queued_for_assignment'
        AND $3->'lifecycle'=o.result->'task'
        AND $2-ARRAY['stage','status','waiting_reason','runtime_state','lifecycle']
            =$3-ARRAY['stage','status','waiting_reason','runtime_state','lifecycle'])
$$;

-- Extend the installed guard at one checked anchor, preserving every legacy and
-- critical-review branch. This is not a replacement of published migration SQL.
DO $$
DECLARE source text; anchor text:=E'BEGIN\n'; replacement text;
BEGIN
    source:=pg_get_functiondef('sdlc_delivery_canonical_gate()'::regprocedure);
    replacement:=E'BEGIN\n    IF sdlc_configured_start_exact(OLD.task_id,OLD.state,NEW.state) THEN RETURN NEW; END IF;\n';
    IF (length(source)-length(replace(source,E'BEGIN\n','')))/length(E'BEGIN\n')
        +(length(source)-length(replace(source,E'BEGIN\r\n','')))/length(E'BEGIN\r\n')<>1 THEN
        RAISE EXCEPTION 'SDLC initial start guard anchor changed';
    END IF;
    IF strpos(source,E'BEGIN\r\n')>0 THEN
        anchor:=E'BEGIN\r\n'; replacement:=replace(replacement,E'\n',E'\r\n');
    END IF;
    EXECUTE replace(source,anchor,replacement);
END $$;

CREATE FUNCTION sdlc_configured_start_committed() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks; intent sdlc_analysis_intents; consent jsonb; expected_intent jsonb; envelope jsonb;
BEGIN
    IF NEW.command IS NULL THEN RETURN NEW; END IF;
    SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=NEW.task_id;
    SELECT * INTO intent FROM sdlc_analysis_intents WHERE task_id=t.task_id
        AND confirmation_id=(NEW.result->'task'->'routing'->>'confirmation_id')::uuid;
    SELECT payload INTO consent FROM sdlc_confirmations WHERE task_id=t.task_id AND id=intent.confirmation_id;
    expected_intent:=jsonb_build_object('contract_version',1,'tracker_instance_id',t.tracker_instance_id,
        'project_id',t.project_id,'task_id',t.task_id,'root_task_id',t.task_id,'intent_id',intent.intent_id,
        'confirmation_id',intent.confirmation_id,'requirement_revision',intent.requirement_revision,
        'content_hash',intent.content_hash,'stage','Analysis','status','Ready','role','Analyst',
        'workflow',NEW.result->'task'->'routing'->'policy'->'routes'->'analyst'->>'workflow_key',
        'mode','analysis','scope','business','cycle',0,'attempt',0,
        'operation_key','analysis:'||intent.confirmation_id::text,'created_at',consent->'created_at');
    envelope:=jsonb_build_object('contract_version',1,'tracker_instance_id',t.tracker_instance_id,
        'project_id',t.project_id,'task_id',t.task_id,'root_task_id',t.task_id,'owner_subject',t.owner_subject,
        'stage','Analysis','requirement_revision',intent.requirement_revision);
    IF NOT ((NEW.transaction_id=pg_current_xact_id() AND t.state->'lifecycle'=NEW.result->'task'
        AND t.state->>'stage'='Analysis' AND t.state->>'status'='Ready'
        AND sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)
        AND EXISTS(SELECT 1 FROM sdlc_delivery_heads h WHERE h.task_id=t.task_id AND h.execution=NEW.result-'task')
        AND intent.routing_snapshot_id::text=NEW.result->'task'->'routing'->>'snapshot_id'
        AND intent.requirement_revision::text=NEW.result->'task'->'routing'->>'requirement_revision'
        AND intent.content_hash=NEW.result->'task'->'routing'->>'content_hash'
        AND intent.payload=expected_intent
        AND EXISTS(SELECT 1 FROM sdlc_outbox e WHERE e.event_id=intent.intent_id AND e.task_id=t.task_id
            AND e.event_type='analysis.intent_created' AND e.payload=envelope||jsonb_build_object('result',intent.payload))
        AND EXISTS(SELECT 1 FROM sdlc_outbox e WHERE e.event_id=NEW.event_id AND e.task_id=t.task_id
            AND e.event_type='delivery.queued' AND e.payload=envelope||jsonb_build_object('result',NEW.result))
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits WHERE task_id=t.task_id AND transaction_id=pg_current_xact_id())
        AND NOT EXISTS(SELECT 1 FROM sdlc_pm_state_write_permits WHERE task_id=t.task_id AND transaction_id=pg_current_xact_id())) IS TRUE) THEN
        RAISE EXCEPTION 'SDLC start must commit exact intent/queue/outbox with consumed permits' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_configured_start_committed AFTER INSERT ON sdlc_delivery_operations
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_configured_start_committed();
