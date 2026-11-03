-- Nullable history witness: old receipts are not rewritten or automatically queued.
ALTER TABLE sdlc_configured_pm_completions ADD COLUMN transaction_id xid8;
ALTER TABLE sdlc_configured_pm_completions ALTER COLUMN transaction_id SET DEFAULT pg_current_xact_id();

-- Keep the existing review journal, command guards and historical receipts.
DO $$
DECLARE definition text; anchor text; replacement text;
BEGIN
    definition:=replace(pg_get_functiondef('sdlc_critical_pm_lifecycle_insert_gate()'::regprocedure), E'\r\n', E'\n');
    anchor:=$anchor$next_task:=jsonb_set(next_task,'{stage}','"Backlog"'::jsonb);
        next_task:=jsonb_set(next_task,'{status}','"Ready"'::jsonb);
        next_task:=jsonb_set(next_task,'{runtime}','"terminal"'::jsonb);
        next_task:=jsonb_set(next_task,'{waiting_reason}','null'::jsonb);$anchor$;
    anchor:=replace(anchor,E'\r\n',E'\n');
    replacement:=replace(replace(replace(anchor,'"Backlog"','"Analysis"'),'"terminal"','"queued"'), '''null''::jsonb','''"queued_for_assignment"''::jsonb');
    IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'configured review queue anchor changed';
    END IF;
    EXECUTE replace(definition,anchor,replacement);

    definition:=replace(pg_get_functiondef('sdlc_critical_pm_lifecycle_commit_gate()'::regprocedure), E'\r\n', E'\n');
    anchor:=$anchor$AND NEW.after_state->>'stage'='Backlog' AND NEW.after_state->>'status'='Ready'
                AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents i WHERE i.task_id=g.task_id
                    AND i.confirmation_id=g.confirmation_id)$anchor$;
    replacement:=$replacement$AND NEW.after_state->>'stage'='Analysis' AND NEW.after_state->>'status'='Ready'
                AND EXISTS(SELECT 1 FROM sdlc_analysis_intents i JOIN sdlc_outbox e ON e.event_id=i.intent_id
                    WHERE i.task_id=g.task_id AND i.review_routing_snapshot_id=g.snapshot_id
                    AND i.confirmation_id=g.confirmation_id AND i.requirement_revision=g.requirement_revision
                    AND i.content_hash=g.content_hash AND e.event_type='analysis.intent_created' AND e.payload->'result'=i.payload)$replacement$;
    anchor:=replace(anchor,E'\r\n',E'\n');
    replacement:=replace(replacement,E'\r\n',E'\n');
    IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'configured review intent anchor changed';
    END IF;
    EXECUTE replace(definition,anchor,replacement);

    definition:=replace(pg_get_functiondef('sdlc_configured_cli_completion_transition()'::regprocedure), E'\r\n', E'\n');
    anchor:='NEW.state->>''stage''=''Backlog'' AND NEW.state->>''status''=''Ready''';
    -- Only the first occurrence belongs to the exact critical-journal branch.
    IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'configured review transition anchor changed'; END IF;
    EXECUTE overlay(definition placing replace(anchor,'''Backlog''','''Analysis''') from position(anchor IN definition) for length(anchor));
END $$;

CREATE OR REPLACE FUNCTION sdlc_configured_cli_completion_committed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=NEW.task_id
        AND sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)
        AND ((t.state->>'stage'='Backlog' AND t.state->>'status'='Ready') OR (
            t.state->>'stage'='Analysis' AND t.state->>'status'='Ready'
            AND t.state->'lifecycle'->>'runtime'='queued'
            AND EXISTS(SELECT 1 FROM sdlc_analysis_intents i
                WHERE i.task_id=t.task_id AND i.confirmation_id=NEW.confirmation_id
                AND i.requirement_revision=NEW.requirement_revision AND i.content_hash=NEW.content_hash)
            AND (EXISTS(SELECT 1 FROM sdlc_delivery_operations o WHERE o.task_id=t.task_id
                    AND o.actor_subject=NEW.actor_subject AND o.transaction_id=NEW.transaction_id
                    AND o.command->>'pm_completion_id'=NEW.completion_id::text
                    AND o.idempotency_key='pm-analysis:'||NEW.completion_id::text)
                OR EXISTS(SELECT 1 FROM sdlc_critical_pm_lifecycle_operations j
                    WHERE j.configured_completion_id=NEW.completion_id AND j.transaction_id=NEW.transaction_id
                    AND j.result=NEW.result AND j.after_state=t.state))))) THEN
        RAISE EXCEPTION 'configured completion must publish exact backlog or same-transaction analysis' USING ERRCODE='23514';
    END IF;
    IF EXISTS(SELECT 1 FROM sdlc_critical_pm_reviews WHERE execution_id=NEW.execution_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_critical_pm_lifecycle_operations j
            WHERE j.execution_id=NEW.execution_id AND j.kind='complete'
                AND j.transaction_id=pg_current_xact_id()
                AND j.configured_completion_id=NEW.completion_id AND j.result=NEW.result) THEN
        RAISE EXCEPTION 'configured review completion requires atomic resolution and routing generation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION sdlc_configured_start_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks; c sdlc_configured_pm_completions; r sdlc_task_routing_snapshots; expected jsonb;
BEGIN
    IF NEW.command IS NULL THEN RETURN NEW; END IF;
    SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT * INTO c FROM sdlc_configured_pm_completions
        WHERE completion_id=(NEW.command->>'pm_completion_id')::uuid AND task_id=t.task_id;
    SELECT * INTO r FROM sdlc_task_routing_snapshots WHERE task_id=t.task_id AND confirmation_id=c.confirmation_id;
    IF NOT ((NEW.transaction_id=pg_current_xact_id()
        AND (NEW.actor_subject=t.owner_subject OR
            (NEW.actor_subject=c.actor_subject AND c.transaction_id=pg_current_xact_id()
             AND NEW.idempotency_key='pm-analysis:'||c.completion_id::text)) AND t.root_task_id=t.task_id
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
CREATE OR REPLACE FUNCTION sdlc_configured_start_exact(task uuid,previous_state jsonb,next_state jsonb) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_delivery_operations o
        JOIN sdlc_delivery_projection_permits p ON p.task_id=o.task_id AND p.transaction_id=o.transaction_id
        WHERE o.task_id=$1 AND o.command->>'operation'='start_analysis' AND o.transaction_id=pg_current_xact_id()
        AND p.next_state=$3 AND p.next_execution=o.result-'task'
        AND $2->>'stage'='Backlog' AND $2->>'status'='Ready' AND NOT $2 ? 'lifecycle'
        AND $2->>'interaction_protocol'='configured-cli-v1'
        AND ($2->>'owner_subject'=o.actor_subject OR EXISTS(
            SELECT 1 FROM sdlc_configured_pm_completions c WHERE c.task_id=o.task_id
                AND c.completion_id=(o.command->>'pm_completion_id')::uuid AND c.actor_subject=o.actor_subject
                AND c.transaction_id=o.transaction_id AND o.idempotency_key='pm-analysis:'||c.completion_id::text))
        AND $3->>'stage'='Analysis' AND $3->>'status'='Ready'
        AND $3->>'runtime_state'='queued' AND $3->>'waiting_reason'='queued_for_assignment'
        AND $3->'lifecycle'=o.result->'task'
        AND $2-ARRAY['stage','status','waiting_reason','runtime_state','lifecycle']
            =$3-ARRAY['stage','status','waiting_reason','runtime_state','lifecycle'])
$$;

