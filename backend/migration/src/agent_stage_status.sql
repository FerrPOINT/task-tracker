-- Agent-owned status transitions use existing immutable operations and current
-- assignments. Stage changes still require owner consent, routing and native settlement.
CREATE FUNCTION sdlc_agent_status_projection(task uuid, before_state jsonb, after_state jsonb)
RETURNS boolean LANGUAGE plpgsql STABLE AS $$
DECLARE head jsonb; next_status text := after_state->>'status'; pm boolean;
BEGIN
    IF before_state->>'interaction_protocol' IS DISTINCT FROM 'configured-cli-v1'
        OR (before_state-ARRAY['status','lifecycle']) IS DISTINCT FROM (after_state-ARRAY['status','lifecycle'])
        OR ((before_state->>'status'='Ready' AND next_status='InProgress')
            OR (before_state->>'status'='InProgress' AND next_status='Completed')) IS NOT TRUE THEN
        RETURN false;
    END IF;
    IF before_state ? 'lifecycle' THEN
        IF ((before_state->'lifecycle')-ARRAY['status','version'])
                IS DISTINCT FROM ((after_state->'lifecycle')-ARRAY['status','version'])
            OR (after_state->'lifecycle'->>'version')::bigint
                IS DISTINCT FROM (before_state->'lifecycle'->>'version')::bigint+1
            OR after_state->'lifecycle'->>'status' IS DISTINCT FROM next_status
            OR NOT EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p
                WHERE p.task_id=task AND p.transaction_id=pg_current_xact_id() AND p.next_state=after_state) THEN
            RETURN false;
        END IF;
    ELSIF after_state ? 'lifecycle' THEN
        RETURN false;
    END IF;
    pm := before_state->>'stage'='Draft';
    IF next_status='InProgress' THEN
        IF pm THEN
            RETURN EXISTS(SELECT 1 FROM sdlc_idempotency o JOIN sdlc_tasks t ON t.task_id=o.task_id
                WHERE o.task_id=task AND t.pm_admission_state='reserved'
                    AND o.actor_subject=before_state->'assignment'->>'machine_subject'
                    AND o.result->'command'->'fence'=jsonb_build_object('kind','pm',
                        'assignment_id',before_state->'assignment'->'assignment_id',
                        'execution_id',before_state->'assignment'->'execution_id',
                        'agent_id',before_state->'assignment'->'agent_id',
                        'assignment_version',before_state->'assignment'->'version')
                    AND o.result->'command'->>'idempotency_key'=o.idempotency_key
                    AND o.result->'stage_start'->>'task_id'=task::text
                    AND o.result->'stage_start'->>'stage'='Draft'
                    AND o.result->'stage_start'->>'status'='InProgress'
                    AND o.result->'stage_start'->'execution_id'=before_state->'assignment'->'execution_id'
                    AND o.result->'stage_start'->>'idempotency_key'=o.idempotency_key);
        END IF;
        SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=task;
        RETURN EXISTS(SELECT 1 FROM sdlc_idempotency o JOIN sdlc_delivery_holds h
                ON h.task_id=o.task_id AND h.assignment_id=(head->'assignment'->>'assignment_id')::uuid
            WHERE o.task_id=task AND head->'task'->>'runtime'='running'
                AND head->'task'->>'status'='Ready'
                AND (head->'lease'->>'expires_at')::timestamptz>clock_timestamp()
                AND o.actor_subject=head->'binding'->>'machine_subject'
                AND head->'assignment'->>'machine_subject'=o.actor_subject
                AND o.result->'command'->'fence'=jsonb_build_object('kind','delivery',
                    'assignment_id',head->'assignment'->'assignment_id',
                    'execution_id',head->'assignment'->'execution_id',
                    'fencing_token',head->'assignment'->'fencing_token',
                    'lease_id',head->'lease'->'lease_id','expected_lease_version',head->'lease'->'version')
                AND o.result->'command'->>'idempotency_key'=o.idempotency_key
                AND o.result->'stage_start'->>'task_id'=task::text
                AND o.result->'stage_start'->'stage'=before_state->'stage'
                AND o.result->'stage_start'->>'status'='InProgress'
                AND o.result->'stage_start'->'execution_id'=head->'assignment'->'execution_id'
                AND o.result->'stage_start'->>'idempotency_key'=o.idempotency_key);
    END IF;
    IF pm THEN
        RETURN EXISTS(SELECT 1 FROM sdlc_configured_pm_completions c JOIN sdlc_tasks t ON t.task_id=c.task_id
            WHERE c.task_id=task AND c.execution_id=t.pm_execution_id AND t.pm_admission_state='reserved'
                AND c.actor_subject=before_state->'assignment'->>'machine_subject'
                AND c.result->>'stage'='Draft' AND c.result->>'status'='Completed'
                AND c.result->'fence'->'assignment_id'=before_state->'assignment'->'assignment_id'
                AND c.result->'fence'->'assignment_version'=before_state->'assignment'->'version'
                AND c.requirement_revision::text=before_state->'revisions'->-1->>'revision'
                AND c.content_hash=before_state->'revisions'->-1->>'content_hash');
    END IF;
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=task;
    RETURN EXISTS(SELECT 1 FROM sdlc_delivery_proposals p
        JOIN sdlc_delivery_proposal_operations o ON o.task_id=p.task_id AND o.result=p.payload
        WHERE p.task_id=task AND p.assignment_id=(head->'assignment'->>'assignment_id')::uuid
            AND head->'task'->>'runtime'='running' AND head->'task'->>'status'='InProgress'
            AND head->'binding'->>'machine_subject'=o.actor_subject
            AND p.payload->>'producer_subject'=o.actor_subject);
END $$;

-- Narrow status-only projections do not change any frozen review or routing fields.
DO $patch$
DECLARE name text; source text; prefix text;
BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_delivery_canonical_gate()','sdlc_critical_projection_gate()',
            'sdlc_configured_cli_completion_transition()'] LOOP
        source:=replace(pg_get_functiondef(to_regprocedure(name)),E'\r\n',E'\n');
        prefix:=E'BEGIN\n    IF TG_TABLE_NAME=''sdlc_tasks'' THEN\n        IF TG_OP=''UPDATE'' AND sdlc_agent_status_projection(OLD.task_id,OLD.state,NEW.state) THEN RETURN NEW; END IF;\n    END IF;\n';
        IF position(E'BEGIN\n' IN source)=0 THEN RAISE EXCEPTION 'agent status guard anchor changed: %',name; END IF;
        EXECUTE overlay(source placing prefix FROM position(E'BEGIN\n' IN source) FOR length(E'BEGIN\n'));
    END LOOP;
END $patch$;

-- Preserve all receipt/identity/fence/native checks; change only the new terminal status contract.
DO $patch$
DECLARE source text; old text;
BEGIN
    source:=replace(pg_get_functiondef('sdlc_configured_pm_completion_gate()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$NEW.result->>'stage'='Backlog' AND NEW.result->>'status'='Ready'$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'PM completion result anchor changed';
    END IF;
    EXECUTE replace(source,old,$anchor$NEW.result->>'stage'='Draft' AND NEW.result->>'status'='Completed'$anchor$);
    source:=replace(pg_get_functiondef('sdlc_delivery_result_gate()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$head->'task'->>'status'='InProgress'$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'delivery native result status anchor changed';
    END IF;
    source:=replace(source,old,$anchor$head->'task'->>'status'='Completed'$anchor$);
    old:=$anchor$IF expected_cycle=3 THEN expected_stage := head->'task'->>'stage';$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'delivery rework limit status anchor changed';
    END IF;
    EXECUTE replace(source,old,$anchor$IF expected_cycle=3 THEN expected_stage := head->'task'->>'stage'; expected_status := 'Completed';$anchor$);
END $patch$;

-- Reuse the published initial-start journal and its exact permit/commit guards.
DO $patch$
DECLARE source text; old text; new text;
BEGIN
    source:=replace(pg_get_functiondef('sdlc_configured_start_insert_gate()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$t.state->>'stage'='Backlog' AND t.state->>'status'='Ready'$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'configured start source status anchor changed';
    END IF;
    source:=replace(source,old,$anchor$t.state->>'stage'='Draft' AND t.state->>'status'='Completed'$anchor$);
    old:=$anchor$c.result->>'stage'='Backlog' AND c.result->>'status'='Ready'$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'configured start receipt status anchor changed';
    END IF;
    EXECUTE replace(source,old,$anchor$c.result->>'stage'='Draft' AND c.result->>'status'='Completed'$anchor$);

    source:=replace(pg_get_functiondef('sdlc_configured_start_exact(uuid,jsonb,jsonb)'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$$2->>'stage'='Backlog' AND $2->>'status'='Ready'$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'configured exact start source anchor changed';
    END IF;
    source:=replace(source,old,$anchor$$2->>'stage'='Draft' AND $2->>'status'='Completed'$anchor$);
    old:=$anchor$$2-ARRAY['stage','status','waiting_reason','runtime_state','lifecycle']$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'configured exact start projection anchor changed';
    END IF;
    source:=replace(source,old,$anchor$$2-ARRAY['stage','status','waiting_reason','runtime_state','lifecycle','draft_submode']$anchor$);
    old:=$anchor$$3->>'runtime_state'='queued'$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'configured exact start target anchor changed';
    END IF;
    EXECUTE replace(source,old,$anchor$NOT $3 ? 'draft_submode' AND $3->>'runtime_state'='queued'$anchor$);

    source:=replace(pg_get_functiondef('sdlc_configured_cli_completion_transition()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$    IF NEW.state->>'stage'='Draft' THEN$anchor$;
    new:=$anchor$    IF TG_OP='UPDATE' AND OLD.state=NEW.state AND NOT NEW.state ? 'lifecycle'
        AND OLD.pm_admission_state='reserved' AND NEW.pm_admission_state='completed'
        AND NEW.state->>'interaction_protocol'='configured-cli-v1'
        AND NEW.state->>'stage'='Draft' AND NEW.state->>'status'='Completed'
        AND sdlc_current_configured_pm_completed(NEW.task_id,NEW.pm_execution_id,NEW.state,NEW.pm_admission_state)
        AND EXISTS(SELECT 1 FROM sdlc_configured_pm_completions c
            WHERE c.task_id=NEW.task_id AND c.execution_id=NEW.pm_execution_id
                AND c.transaction_id=pg_current_xact_id() AND c.actor_subject=NEW.state->'assignment'->>'machine_subject'
                AND c.result->'owner_version'=to_jsonb(NEW.pm_owner_version)
                AND c.result->>'stage'='Draft' AND c.result->>'status'='Completed') THEN
        RETURN NEW;
    END IF;
    IF NEW.state->>'stage'='Draft' THEN$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'configured completed admission anchor changed';
    END IF;
    EXECUTE replace(source,old,new);

    -- Keep 112's same-transaction start/review correlation and all deferred checks.
    source:=replace(pg_get_functiondef('sdlc_configured_cli_completion_committed()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$((t.state->>'stage'='Backlog' AND t.state->>'status'='Ready') OR ($anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'configured completion commit target anchor changed';
    END IF;
    EXECUTE replace(source,old,'((');
END $patch$;

-- Review commands retain agent status, not the former owner-consent status side effects.
-- Frozen assignment, owner, blocker, routing generation and journal checks remain unchanged.
DO $patch$
DECLARE source text; old text; new text;
BEGIN
    source:=replace(pg_get_functiondef('sdlc_critical_pm_lifecycle_insert_gate()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$    expected:=jsonb_set(expected,'{lifecycle}',next_task);$anchor$;
    new:=$anchor$    IF configured IS TRUE AND NEW.kind='command' THEN
        next_task:=jsonb_set(next_task,'{status}',NEW.before_state->'status');
    END IF;
    expected:=jsonb_set(expected,'{lifecycle}',next_task);$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'review status projection anchor changed';
    END IF;
    EXECUTE replace(source,old,new);
END $patch$;
