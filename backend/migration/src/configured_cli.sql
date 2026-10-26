-- The marker is opt-in on creation only; existing aggregates and ledgers are untouched.
DO $$
DECLARE c record; definition text; changed text; n integer := 0;
BEGIN
    FOR c IN SELECT oid,conname FROM pg_constraint
        WHERE conrelid='sdlc_draft_creations'::regclass AND contype='c'
    LOOP
        definition := pg_get_constraintdef(c.oid);
        IF position('result - ARRAY[' IN definition)>0 THEN
            changed := replace(definition,'result - ARRAY[','(result - ''interaction_protocol'') - ARRAY[');
            EXECUTE format('ALTER TABLE sdlc_draft_creations DROP CONSTRAINT %I',c.conname);
            EXECUTE format('ALTER TABLE sdlc_draft_creations ADD CONSTRAINT %I %s',c.conname,changed);
            n := n+1;
        END IF;
    END LOOP;
    IF n<>1 THEN RAISE EXCEPTION 'configured CLI creation shape anchor changed'; END IF;
END $$;
ALTER TABLE sdlc_draft_creations ADD CONSTRAINT sdlc_creation_protocol CHECK
    ((NOT result ? 'interaction_protocol' OR result->'interaction_protocol'='"configured-cli-v1"'::jsonb) IS TRUE);

CREATE FUNCTION sdlc_configured_cli_marker_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='UPDATE' AND OLD.state->'interaction_protocol' IS DISTINCT FROM NEW.state->'interaction_protocol' THEN
        RAISE EXCEPTION 'interaction protocol is immutable; no legacy retrofit' USING ERRCODE='23514';
    END IF;
    IF NOT NEW.state ? 'interaction_protocol' THEN RETURN NEW; END IF;
    IF NOT ((NEW.state->'interaction_protocol'='"configured-cli-v1"'::jsonb
        AND NEW.state->'flow'='"single-task-v1"'::jsonb) IS TRUE) THEN
        RAISE EXCEPTION 'invalid configured CLI protocol' USING ERRCODE='23514';
    END IF;
    IF TG_OP='UPDATE' AND OLD.pm_execution_id IS NOT NULL
        AND OLD.state->'workflow_task_key' IS DISTINCT FROM NEW.state->'workflow_task_key' THEN
        RAISE EXCEPTION 'configured CLI workflow business key is immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.pm_execution_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM sdlc_pm_executions e WHERE e.task_id=NEW.task_id AND e.execution_id=NEW.pm_execution_id
            AND e.result->'interaction_protocol'=NEW.state->'interaction_protocol'
            AND e.result->'assignment'=NEW.state->'assignment'
            AND NEW.state->>'workflow_task_key'='SDLC-'||e.ordinal::text
    ) THEN
        RAISE EXCEPTION 'configured CLI requires exact business reservation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_cli_marker_gate BEFORE INSERT OR UPDATE ON sdlc_tasks
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_marker_gate();

CREATE FUNCTION sdlc_configured_cli_origin_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state ? 'interaction_protocol' AND NOT EXISTS(
        SELECT 1 FROM sdlc_draft_creations c WHERE c.task_id=NEW.task_id AND c.project_id=NEW.project_id
            AND c.actor_subject=NEW.owner_subject AND c.result->'interaction_protocol'=NEW.state->'interaction_protocol'
    ) THEN RAISE EXCEPTION 'configured CLI requires original owner creation ledger' USING ERRCODE='23514'; END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_configured_cli_origin_gate AFTER INSERT ON sdlc_tasks
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_origin_gate();

CREATE FUNCTION sdlc_configured_cli_creation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=NEW.task_id
        AND t.state->'interaction_protocol' IS NOT DISTINCT FROM NEW.result->'interaction_protocol') THEN
        RAISE EXCEPTION 'creation protocol must match original task' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_cli_creation_gate BEFORE INSERT ON sdlc_draft_creations
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_creation_gate();

CREATE FUNCTION sdlc_configured_cli_confirmation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks;
BEGIN
    SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    IF t.state->>'interaction_protocol' IS DISTINCT FROM 'configured-cli-v1' THEN RETURN NEW; END IF;
    IF NOT ((t.state->>'stage'='Draft' AND t.pm_admission_state='reserved'
        AND sdlc_current_pm_draft_reserved(t.task_id,t.pm_execution_id,t.state,t.pm_owner_version,t.pm_admission_state)
        AND t.state->'confirmation_revision'=to_jsonb(NEW.revision)
        AND t.state->'revisions'->-1->'revision'=to_jsonb(NEW.revision)
        AND t.state->'revisions'->-1->>'content_hash'=NEW.content_hash
        AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(t.state->'questions') q
            WHERE q->'required'='true'::jsonb AND q->>'state'='open')
        AND NEW.payload->>'id'=NEW.id::text AND NEW.payload->>'task_id'=NEW.task_id::text
        AND NEW.payload->'revision'=to_jsonb(NEW.revision) AND NEW.payload->>'content_hash'=NEW.content_hash
        AND NEW.payload->>'owner_subject'=t.owner_subject AND NEW.payload->>'stage'='Draft'
        AND jsonb_typeof(NEW.payload->'created_at')='string'
        AND NEW.payload-ARRAY['id','task_id','revision','content_hash','owner_subject','created_at','stage']='{}'::jsonb) IS TRUE) THEN
        RAISE EXCEPTION 'configured consent requires current owner-confirmed business-ready Draft' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_cli_confirmation_gate BEFORE INSERT ON sdlc_confirmations
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_confirmation_gate();

CREATE FUNCTION sdlc_configured_cli_no_native_lease() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.pm_execution_id=NEW.execution_id
        AND t.state->'interaction_protocol'='"configured-cli-v1"'::jsonb) THEN
        RAISE EXCEPTION 'configured CLI has no native execution lease' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_cli_no_native_lease BEFORE INSERT ON sdlc_pm_execution_leases
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_no_native_lease();

CREATE TABLE sdlc_configured_pm_completions (
    completion_id uuid PRIMARY KEY CHECK(completion_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL UNIQUE REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    execution_id uuid NOT NULL UNIQUE REFERENCES sdlc_pm_executions(execution_id) ON DELETE RESTRICT,
    confirmation_id uuid NOT NULL,
    requirement_revision bigint NOT NULL CHECK(requirement_revision BETWEEN 1 AND 9007199254740991),
    content_hash text NOT NULL CHECK(content_hash ~ '^[0-9a-f]{64}$'),
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    payload_hash text NOT NULL CHECK(payload_hash ~ '^[0-9a-f]{64}$'),
    command jsonb NOT NULL,
    workflow_receipt jsonb NOT NULL,
    result jsonb NOT NULL,
    created_at timestamptz NOT NULL,
    FOREIGN KEY(task_id,confirmation_id,requirement_revision,content_hash)
        REFERENCES sdlc_confirmations(task_id,id,revision,content_hash) ON DELETE RESTRICT,
    UNIQUE(task_id,idempotency_key)
);
CREATE TRIGGER sdlc_configured_pm_completion_immutable BEFORE UPDATE OR DELETE ON sdlc_configured_pm_completions
    FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
DO $$
DECLARE definition text; anchor text := '''sdlc_idempotency'',''sdlc_outbox'',';
BEGIN
    definition:=pg_get_functiondef('tracker_authorize_native_sdlc(text,text,jsonb,jsonb)'::regprocedure);
    IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'configured completion native task guard anchor changed';
    END IF;
    EXECUTE replace(definition,anchor,anchor||'''sdlc_configured_pm_completions'',');
END $$;
CREATE TRIGGER namespace_write_guard BEFORE INSERT OR UPDATE OR DELETE ON sdlc_configured_pm_completions
    FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard();
CREATE CONSTRAINT TRIGGER shared_sdlc_commit_clock AFTER INSERT OR UPDATE OR DELETE ON sdlc_configured_pm_completions
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_sdlc_commit_clock();

CREATE FUNCTION sdlc_configured_pm_completion_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks; e sdlc_pm_executions; c sdlc_confirmations;
BEGIN
    SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT * INTO STRICT e FROM sdlc_pm_executions WHERE execution_id=NEW.execution_id;
    SELECT * INTO STRICT c FROM sdlc_confirmations WHERE id=NEW.confirmation_id;
    IF NOT ((t.state->>'interaction_protocol'='configured-cli-v1'
        AND t.root_task_id=t.task_id AND t.pm_execution_id=NEW.execution_id AND t.pm_admission_state='reserved'
        AND e.task_id=t.task_id AND e.owner_version=t.pm_owner_version AND e.result->'assignment'=t.state->'assignment'
        AND e.result->>'interaction_protocol'='configured-cli-v1'
        AND t.state->>'workflow_task_key'='SDLC-'||e.ordinal::text
        AND t.state->>'stage'='Draft' AND t.state->>'status'='InProgress'
        AND t.state->'confirmation_revision'=to_jsonb(NEW.requirement_revision)
        AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(t.state->'questions') q
            WHERE q->'required'='true'::jsonb AND q->>'state'='open')
        AND t.state->'revisions'->-1->'revision'=to_jsonb(NEW.requirement_revision)
        AND t.state->'revisions'->-1->>'content_hash'=NEW.content_hash
        AND t.state->'assignment'->>'machine_subject'=NEW.actor_subject
        AND c.task_id=t.task_id AND c.revision=NEW.requirement_revision AND c.content_hash=NEW.content_hash
        AND c.payload->>'owner_subject'=t.owner_subject AND c.payload->>'stage'='Draft'
        AND t.state->'confirmations' @> jsonb_build_array(c.payload)
        AND NEW.command->>'interaction_protocol'='configured-cli-v1'
        AND NEW.command->>'idempotency_key'=NEW.idempotency_key
        AND NEW.command->'requirement_revision'=to_jsonb(NEW.requirement_revision)
        AND NEW.command->>'content_hash'=NEW.content_hash
        AND NEW.command->'fence'=jsonb_build_object('assignment_id',e.assignment_id,'execution_id',e.execution_id,
            'agent_id',t.state->'assignment'->'agent_id','assignment_version',e.assignment_version)
        AND NEW.workflow_receipt->>'interaction_protocol'='configured-cli-v1'
        AND NEW.workflow_receipt->>'tracker_task_id'=t.task_id::text
        AND NEW.workflow_receipt->>'task'=t.state->>'workflow_task_key'
        AND NEW.workflow_receipt->'namespace_id'=NEW.command->'namespace_id'
        AND NEW.workflow_receipt->'execution_ref'=NEW.command->'workflow_execution_ref'
        AND NEW.workflow_receipt->'operation_key'=NEW.command->'workflow_operation_key'
        AND NEW.workflow_receipt->'result_version'=NEW.command->'workflow_result_version'
        AND NEW.workflow_receipt->'requirements_revision'=to_jsonb(NEW.requirement_revision)
        AND NEW.workflow_receipt->>'requirements_sha256'=NEW.content_hash
        AND NEW.workflow_receipt->>'confirmation_id'=NEW.confirmation_id::text
        AND NEW.workflow_receipt->>'owner_subject'=t.owner_subject
        AND NEW.workflow_receipt->>'assignment_ref'=e.assignment_id::text
        AND NEW.workflow_receipt->>'tracker_execution_id'=e.execution_id::text
        AND NEW.workflow_receipt->'assignment_version'=to_jsonb(e.assignment_version)
        AND NEW.workflow_receipt->'concrete_agent_ref'=t.state->'assignment'->'agent_id'
        AND NEW.workflow_receipt->>'machine_subject'=NEW.actor_subject
        AND NEW.workflow_receipt->>'status'='done'
        AND NEW.result->>'completion_id'=NEW.completion_id::text
        AND NEW.result->>'task_id'=t.task_id::text AND NEW.result->>'root_task_id'=t.root_task_id::text
        AND NEW.result->>'tracker_instance_id'=t.tracker_instance_id AND NEW.result->>'project_id'=t.project_id::text
        AND NEW.result->>'owner_subject'=t.owner_subject AND NEW.result->'fence'=NEW.command->'fence'
        AND NEW.result->'owner_version'=to_jsonb(t.pm_owner_version)
        AND NEW.result->'requirement_revision'=to_jsonb(NEW.requirement_revision) AND NEW.result->>'content_hash'=NEW.content_hash
        AND NEW.result->>'confirmation_id'=NEW.confirmation_id::text AND NEW.result->'workflow_receipt'=NEW.workflow_receipt
        AND NEW.result->>'workflow_receipt_sha256' ~ '^[0-9a-f]{64}$'
        AND NEW.result->>'idempotency_key'=NEW.idempotency_key AND NEW.result->>'interaction_protocol'='configured-cli-v1'
        AND NEW.result->>'stage'='Backlog' AND NEW.result->>'status'='Ready'
        AND NEW.result->>'previous_pm_admission_state'='reserved' AND NEW.result->>'next_pm_admission_state'='completed'
        AND (NEW.result->>'created_at')::timestamptz=NEW.created_at) IS TRUE) THEN
        RAISE EXCEPTION 'configured completion requires current assignment and exact owner-confirmed terminal receipt' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_pm_completion_gate BEFORE INSERT ON sdlc_configured_pm_completions
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_pm_completion_gate();

CREATE FUNCTION sdlc_current_configured_pm_completed(task uuid,execution uuid,state jsonb,admission_state text)
RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT (admission_state='completed' AND state->>'interaction_protocol'='configured-cli-v1'
        AND EXISTS(SELECT 1 FROM sdlc_configured_pm_completions c JOIN sdlc_confirmations consent
            ON consent.task_id=c.task_id AND consent.id=c.confirmation_id
            WHERE c.task_id=task AND c.execution_id=execution AND c.result->'fence'->'assignment_id'=state->'assignment'->'assignment_id'
            AND c.result->'fence'->'assignment_version'=state->'assignment'->'version'
            AND c.result->'fence'->'agent_id'=state->'assignment'->'agent_id'
            AND c.requirement_revision::text=state->'revisions'->-1->>'revision'
            AND c.content_hash=state->'revisions'->-1->>'content_hash'
            AND consent.payload->>'stage'='Draft' AND consent.payload->>'owner_subject'=state->>'owner_subject'
            AND state->'confirmations' @> jsonb_build_array(consent.payload)
            AND c.result->>'next_pm_admission_state'='completed')) IS TRUE
$$;

CREATE FUNCTION sdlc_configured_cli_completion_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state->>'interaction_protocol' IS DISTINCT FROM 'configured-cli-v1' THEN RETURN NEW; END IF;
    IF NEW.state->>'stage'='Draft' THEN
        IF NEW.pm_admission_state='completed' THEN
            RAISE EXCEPTION 'configured completed admission must publish Backlog' USING ERRCODE='23514';
        END IF;
        IF TG_OP='UPDATE' AND OLD.state->>'stage' IS DISTINCT FROM 'Draft' THEN
            RAISE EXCEPTION 'configured completion cannot revert to Draft' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP='INSERT' THEN RAISE EXCEPTION 'configured task must start in Draft' USING ERRCODE='23514'; END IF;
    IF NOT ((NEW.state->>'stage'='Backlog' AND NEW.state->>'status'='Ready'
        AND NOT NEW.state ? 'draft_submode'
        AND sdlc_current_configured_pm_completed(NEW.task_id,NEW.pm_execution_id,NEW.state,NEW.pm_admission_state)
        AND ((OLD.state->>'stage'='Draft' AND OLD.pm_admission_state='reserved'
            AND OLD.state-ARRAY['stage','status','draft_submode']=NEW.state-ARRAY['stage','status','draft_submode'])
            OR (OLD.state=NEW.state AND OLD.pm_admission_state=NEW.pm_admission_state))) IS TRUE) THEN
        RAISE EXCEPTION 'configured publication requires exact terminal completion; no Analysis' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_cli_completion_transition BEFORE INSERT OR UPDATE ON sdlc_tasks
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_completion_transition();

CREATE FUNCTION sdlc_configured_cli_completion_committed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=NEW.task_id
        AND t.state->>'stage'='Backlog' AND t.state->>'status'='Ready'
        AND sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)) THEN
        RAISE EXCEPTION 'configured completion must atomically publish Backlog' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_configured_cli_completion_committed AFTER INSERT ON sdlc_configured_pm_completions
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_completion_committed();

-- Configured publication has its own exact completion ledger, not legacy routing/native evidence.
DO $$
DECLARE definition text; anchor text := 'IF NEW.state ? ''flow'' AND NEW.state->>''stage'' NOT IN (''Draft'',''Clarification'')';
BEGIN
    definition:=pg_get_functiondef('sdlc_draft_flow_gate()'::regprocedure);
    IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'configured completion Draft flow anchor changed';
    END IF;
    EXECUTE replace(definition,anchor,anchor||' AND NOT sdlc_current_configured_pm_completed(NEW.task_id,NEW.pm_execution_id,NEW.state,NEW.pm_admission_state)');
END $$;
-- Preserve the installed legacy definition and its OID; add only the new exact ledger alternative.
DO $$
DECLARE definition text; anchor text := 'SELECT admission_state=''completed''';
BEGIN
    definition:=pg_get_functiondef('sdlc_current_pm_completed(uuid,uuid,jsonb,text)'::regprocedure);
    IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'configured completion legacy function anchor changed';
    END IF;
    EXECUTE replace(definition,anchor,'SELECT sdlc_current_configured_pm_completed(task,execution,state,admission_state) OR admission_state=''completed''');
END $$;
