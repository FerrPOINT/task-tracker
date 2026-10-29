-- An execution has one immutable completion; a business task may have a later PM
-- review. Keep every old receipt and operation key instead of replacing history.
ALTER TABLE sdlc_configured_pm_completions
    DROP CONSTRAINT sdlc_configured_pm_completions_task_id_key;
CREATE INDEX sdlc_configured_pm_completion_task
    ON sdlc_configured_pm_completions(task_id,created_at);

CREATE OR REPLACE FUNCTION sdlc_configured_cli_marker_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='UPDATE' AND OLD.state->'interaction_protocol' IS DISTINCT FROM NEW.state->'interaction_protocol' THEN
        RAISE EXCEPTION 'interaction protocol requires explicit adoption; no implicit retrofit' USING ERRCODE='23514';
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
            AND NEW.state->>'workflow_task_key'=(SELECT root.result->'execution'->>'key'
                FROM sdlc_pm_executions root WHERE root.task_id=NEW.task_id ORDER BY root.ordinal LIMIT 1)
    ) THEN
        RAISE EXCEPTION 'configured CLI requires exact business reservation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

-- The receipt belongs to the immutable business key, not the ordinal allocated
-- to a later PM review. All assignment, consent and Workflow receipt checks stay.
DO $$
DECLARE definition text; anchor text := 't.state->>''workflow_task_key''=''SDLC-''||e.ordinal::text';
BEGIN
    definition:=pg_get_functiondef('sdlc_configured_pm_completion_gate()'::regprocedure);
    IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'configured completion business key anchor changed';
    END IF;
    EXECUTE replace(definition,anchor,
        't.state->>''workflow_task_key''=(SELECT root.result->''execution''->>''key'' FROM sdlc_pm_executions root WHERE root.task_id=t.task_id ORDER BY root.ordinal LIMIT 1)');
END $$;

CREATE OR REPLACE FUNCTION sdlc_configured_cli_completion_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state->>'interaction_protocol' IS DISTINCT FROM 'configured-cli-v1' THEN RETURN NEW; END IF;
    -- Technical progression is owned by the existing lifecycle transaction. Its
    -- exact projection permit and all lifecycle/result/consent guards still run.
    -- This branch cannot start Analyst as a side effect of PM publication.
    IF TG_OP='UPDATE' AND NEW.state ? 'lifecycle'
        AND sdlc_current_configured_pm_completed(OLD.task_id,OLD.pm_execution_id,OLD.state,OLD.pm_admission_state)
        AND sdlc_current_configured_pm_completed(NEW.task_id,NEW.pm_execution_id,NEW.state,NEW.pm_admission_state)
        AND EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p
            WHERE p.task_id=NEW.task_id AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN
        RETURN NEW;
    END IF;
    IF NEW.state->>'stage'='Draft' THEN
        IF NEW.pm_admission_state='completed' THEN
            RAISE EXCEPTION 'configured completed admission requires a recorded lifecycle transition' USING ERRCODE='23514';
        END IF;
        IF TG_OP='UPDATE' AND OLD.state->>'stage' IS DISTINCT FROM 'Draft' THEN
            RAISE EXCEPTION 'configured return to Draft requires a recorded lifecycle transition' USING ERRCODE='23514';
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
        RAISE EXCEPTION 'configured publication requires exact terminal completion; lifecycle requires its own permit' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

-- Preserve the historical review shape while accepting the same configured
-- protocol marker as the current PM reservation. Do not change old ledger rows.
DO $$
DECLARE constraint_row record; removed integer := 0;
BEGIN
    FOR constraint_row IN SELECT conname FROM pg_constraint
        WHERE conrelid='sdlc_critical_pm_reviews'::regclass AND contype='c'
            AND position('critical_business_review' IN pg_get_constraintdef(oid))>0
    LOOP
        EXECUTE format('ALTER TABLE sdlc_critical_pm_reviews DROP CONSTRAINT %I',constraint_row.conname);
        removed := removed + 1;
    END LOOP;
    IF removed<>1 THEN RAISE EXCEPTION 'critical PM review shape anchor changed'; END IF;
END $$;
ALTER TABLE sdlc_critical_pm_reviews ADD CONSTRAINT sdlc_critical_pm_review_shape
    CHECK ((command-ARRAY['blocker_id','blocker_sha256','expected_blocker_version','expected_task_version',
            'expected_pm_owner_version','expected_pm_assignment_version','idempotency_key']='{}'::jsonb
        AND result-ARRAY['purpose','reservation','context']='{}'::jsonb
        AND (result->'reservation')-ARRAY['interaction_protocol','contract_version','variant','binding','owner_cas','assignment',
            'execution','input','assignment_operation_key','admission_state','dispatch_allowed']='{}'::jsonb
        AND (result->'reservation'->'assignment')-ARRAY['assignment_id','execution_id','agent_id','version','machine_subject']='{}'::jsonb
        AND (result->'reservation'->'execution')-ARRAY['ordinal','key']='{}'::jsonb
        AND result->>'purpose'='critical_business_review'
        AND result->'reservation'->'contract_version'='1'::jsonb
        AND result->'reservation'->>'variant'='pm_draft_reserved'
        AND result->'reservation'->>'admission_state'='reserved'
        AND result->'reservation'->'dispatch_allowed'='false'::jsonb
        AND result->'reservation'->'binding'->>'task_id'=task_id::text
        AND result->'reservation'->'binding'->>'root_task_id'=task_id::text
        AND result->'reservation'->'assignment'->>'execution_id'=execution_id::text
        AND result->'reservation'->'assignment'->>'assignment_id'=assignment_id::text
        AND result->'reservation'->'assignment'->'version'=to_jsonb(assignment_version)
        AND result->'reservation'->'owner_cas'=jsonb_build_object(
            'expected_version',previous_owner_version,'version',owner_version)
        AND result->'context'->>'blocker_id'=blocker_id::text
        AND after_blocker->>'blocker_id'=blocker_id::text
        AND after_blocker->>'state'='in_pm_review'
        AND after_blocker->'pm_review'=jsonb_build_object('assignment_id',assignment_id,'execution_id',execution_id)
        AND command->>'idempotency_key'=idempotency_key
        AND command->>'blocker_id'=blocker_id::text
        AND command->'expected_pm_owner_version'=to_jsonb(previous_owner_version)
        AND after_state->'assignment'=result->'reservation'->'assignment'
        AND before_execution=after_execution
        AND after_execution=jsonb_build_object('assignment',NULL,'lease',NULL,'binding',NULL,'result',NULL)) IS TRUE);
ALTER TABLE sdlc_critical_pm_reviews ADD CONSTRAINT sdlc_critical_pm_review_protocol CHECK
    ((NOT (result->'reservation') ? 'interaction_protocol'
        OR result->'reservation'->'interaction_protocol'='"configured-cli-v1"'::jsonb) IS TRUE);
