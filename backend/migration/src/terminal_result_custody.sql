-- Resolve result custody without restoring a reconciled assignment to the head.
-- The original terminal operation and every historical receipt remain immutable.
CREATE FUNCTION sdlc_delivery_result_custody(task uuid, assignment uuid)
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE head jsonb; original jsonb; binding jsonb; reconciliation jsonb;
    stopped jsonb; original_lease jsonb; aggregate_state jsonb;
BEGIN
    SELECT t.state,d.receipt INTO aggregate_state,head
        FROM sdlc_tasks t JOIN sdlc_delivery_tasks d USING(task_id)
        WHERE t.task_id=task FOR UPDATE OF t;
    IF head IS NULL THEN
        RAISE EXCEPTION 'SDLC result custody absent' USING ERRCODE='23514';
    END IF;
    IF head->'assignment'<>'null'::jsonb THEN
        IF head#>>'{assignment,assignment_id}' IS DISTINCT FROM assignment::text
            OR NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds h
                WHERE h.task_id=task AND h.assignment_id=assignment) THEN
            RAISE EXCEPTION 'SDLC result custody differs' USING ERRCODE='23514';
        END IF;
        RETURN head;
    END IF;
    SELECT a.payload,b.payload,r.payload INTO original,binding,reconciliation
        FROM sdlc_delivery_assignments a
        JOIN sdlc_delivery_run_bindings b USING(assignment_id)
        JOIN sdlc_delivery_reconciliations r USING(assignment_id,task_id)
        WHERE a.assignment_id=assignment AND a.task_id=task;
    -- A bound original operation supplies the archived scheduler holder/version.
    -- DISTINCT admits repeated reads of the same immutable custody, never a guess.
    SELECT DISTINCT o.result->'lease' INTO STRICT original_lease
        FROM sdlc_delivery_operations o WHERE o.task_id=task
        AND o.result->'assignment'=original AND o.result->'binding'=binding
        AND o.result#>>'{task,runtime}'='running'
        AND o.result#>'{lease,version}'=reconciliation#>'{fence,expected_lease_version}';
    SELECT o.result INTO stopped FROM sdlc_delivery_operations o
        WHERE o.task_id=task AND o.idempotency_key=reconciliation->>'idempotency_key'
        AND o.actor_subject=original_lease->>'holder_subject';
    IF (aggregate_state->>'interaction_protocol'='configured-cli-v1'
        AND aggregate_state->>'flow'='single-task-v1'
        AND head=stopped AND head#>>'{task,runtime}'='terminal'
        AND head#>>'{task,status}'='Ready'
        AND head#>>'{task,waiting_reason}'='runtime_terminal_requires_intervention'
        AND head->'lease'='null'::jsonb AND head->'binding'='null'::jsonb AND head->'result'='null'::jsonb
        AND reconciliation->>'operation'='reconcile'
        AND reconciliation#>>'{recovery,disposition}'='stop'
        AND reconciliation#>>'{terminal,state}'='succeeded'
        AND reconciliation#>'{terminal,assignment_hash}'=original->'assignment_hash'
        AND reconciliation#>'{terminal,run_id}'=binding->'run_id'
        AND reconciliation#>'{terminal,chat_id}'=binding->'chat_id'
        AND reconciliation#>'{fence,assignment_id}'=original->'assignment_id'
        AND reconciliation#>'{fence,execution_id}'=original->'execution_id'
        AND reconciliation#>'{fence,fencing_token}'=original->'fencing_token'
        AND reconciliation#>'{fence,lease_id}'=original->'lease_id'
        AND original_lease->'lease_id'=original->'lease_id'
        AND original->'routing'=head#>'{task,routing}'
        AND original->'stage'=head#>'{task,stage}' AND original->'cycle'=head#>'{task,cycle}'
        AND original->'previous_result_id'=head#>'{task,previous_result_id}'
        AND original->'owner_subject'=aggregate_state->'owner_subject'
        AND original#>'{routing,requirement_revision}'=aggregate_state->'revisions'->-1->'revision'
        AND original#>'{routing,content_hash}'=aggregate_state->'revisions'->-1->'content_hash'
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_assignments a WHERE a.task_id=task
            AND (a.payload->>'fencing_token')::bigint>(original->>'fencing_token')::bigint)
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_results r WHERE r.assignment_id=assignment)) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC successful terminal custody is no longer current' USING ERRCODE='23514';
    END IF;
    RETURN head || jsonb_build_object('assignment',original,'binding',binding,'lease',original_lease);
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
    RAISE EXCEPTION 'SDLC original terminal custody is missing or ambiguous' USING ERRCODE='23514';
END $$;

-- Keep the installed trigger owners and their complete typed-result rules.
-- Only custody selection and its corresponding terminal state are extended.
DO $patch$
DECLARE name text; source text; old text; replacement text;
BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_delivery_proposal_gate()', 'sdlc_delivery_result_gate()'] LOOP
        source:=replace(pg_get_functiondef(to_regprocedure(name)),E'\r\n',E'\n');
        old:=$anchor$    SELECT t.receipt,b.payload INTO head,binding FROM sdlc_delivery_tasks t
        JOIN sdlc_delivery_assignments a ON a.task_id=t.task_id
        JOIN sdlc_delivery_run_bindings b USING(assignment_id)
        JOIN sdlc_delivery_holds h USING(assignment_id)
        WHERE t.task_id=NEW.task_id AND a.assignment_id=NEW.assignment_id FOR UPDATE OF t;$anchor$;
        IF source IS NULL OR (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
            RAISE EXCEPTION 'terminal custody trigger source differs: %',name;
        END IF;
        source:=replace(source,old,$anchor$    head:=sdlc_delivery_result_custody(NEW.task_id,NEW.assignment_id);
    binding:=head->'binding';$anchor$);
        IF name='sdlc_delivery_proposal_gate()' THEN
            old:=$anchor$head->'task'->>'runtime'='running' AND head->'task'->>'status'='InProgress'$anchor$;
            replacement:=$anchor$((head#>>'{task,runtime}'='running' AND head#>>'{task,status}'='InProgress')
            OR (head#>>'{task,runtime}'='terminal' AND head#>>'{task,status}'='Ready'))$anchor$;
        ELSE
            old:=$anchor$head->'task'->>'status'='Completed' AND head->'task'->>'runtime'='running'$anchor$;
            replacement:=$anchor$((head#>>'{task,status}'='Completed' AND head#>>'{task,runtime}'='running')
            OR (head#>>'{task,status}'='Ready' AND head#>>'{task,runtime}'='terminal'
                AND EXISTS(SELECT 1 FROM sdlc_delivery_reconciliations r
                    WHERE r.assignment_id=NEW.assignment_id AND r.task_id=NEW.task_id
                    AND r.payload#>'{terminal,receipt}'=NEW.payload#>'{terminal,fleet_receipt}')))$anchor$;
        END IF;
        IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
            RAISE EXCEPTION 'terminal custody trigger state differs: %',name;
        END IF;
        EXECUTE replace(source,old,replacement);
    END LOOP;
END $patch$;
