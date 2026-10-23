-- Append-only recovery of an already consumed input. Initial-unsubmitted and
-- bootstrap classifiers remain unchanged; no old receipt becomes live authority.
LOCK TABLE sdlc_tasks,sdlc_pm_execution_leases,sdlc_pm_resume_inputs,sdlc_pm_lease_operations
    IN ACCESS EXCLUSIVE MODE;

CREATE FUNCTION sdlc_pm_resume_key_namespace_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task uuid;
BEGIN
    IF TG_TABLE_NAME='sdlc_pm_resume_inputs' THEN
        task:=NEW.task_id;
        IF NEW.command ? 'recovery' THEN
            RAISE EXCEPTION 'Recovery must not consume the original input again' USING ERRCODE='23514';
        END IF;
    ELSE
        SELECT task_id INTO STRICT task FROM sdlc_pm_executions WHERE execution_id=NEW.execution_id;
    END IF;
    PERFORM task_id FROM sdlc_tasks WHERE task_id=task FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs WHERE task_id=task AND idempotency_key=NEW.idempotency_key)
        OR EXISTS(SELECT 1 FROM sdlc_pm_lease_operations o JOIN sdlc_pm_executions e USING(execution_id)
            WHERE e.task_id=task AND o.idempotency_key=NEW.idempotency_key) THEN
        RAISE EXCEPTION 'PM resume key already exists in input/lease operation namespace' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
-- Run before the existing input/recovery gates; all writers use task -> lease order.
CREATE TRIGGER sdlc_aa_pm_resume_key_namespace_gate BEFORE INSERT ON sdlc_pm_resume_inputs
FOR EACH ROW EXECUTE FUNCTION sdlc_pm_resume_key_namespace_gate();
CREATE TRIGGER sdlc_aa_pm_resume_key_namespace_gate BEFORE INSERT ON sdlc_pm_lease_operations
FOR EACH ROW EXECUTE FUNCTION sdlc_pm_resume_key_namespace_gate();

-- Exempt only the new discriminator from the old gate, never its old cases.
DROP TRIGGER sdlc_pm_unsubmitted_recovery_gate ON sdlc_pm_lease_operations;
CREATE TRIGGER sdlc_pm_unsubmitted_recovery_gate BEFORE INSERT ON sdlc_pm_lease_operations
FOR EACH ROW WHEN (NEW.recovery IS NOT NULL
    AND NEW.recovery->>'kind' IS DISTINCT FROM 'bootstrap_retry'
    AND NEW.recovery->>'kind' IS DISTINCT FROM 'settled_input_expiry')
EXECUTE FUNCTION sdlc_pm_unsubmitted_recovery_gate();

CREATE UNIQUE INDEX sdlc_pm_settled_input_recovery_predecessor ON sdlc_pm_lease_operations(
    execution_id,(recovery#>>'{command,recovery,original_key}'),
    (recovery#>>'{command,recovery,original_request_sha256}'),
    (recovery#>>'{command,recovery,predecessor_key}'),
    (recovery#>>'{command,recovery,predecessor_request_sha256}'))
WHERE recovery->>'kind'='settled_input_expiry';

CREATE FUNCTION sdlc_pm_settled_input_recovery_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks; l sdlc_pm_execution_leases; root sdlc_pm_resume_inputs;
        c jsonb; anchor jsonb; w jsonb; r jsonb; predecessor jsonb;
        predecessor_key text; predecessor_hash text; attempts bigint;
BEGIN
    c:=NEW.recovery->'command'; anchor:=c->'recovery'; w:=NEW.recovery->'witness';
    SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=(NEW.result#>>'{binding,task_id}')::uuid FOR UPDATE;
    SELECT * INTO STRICT l FROM sdlc_pm_execution_leases WHERE execution_id=NEW.execution_id FOR UPDATE;
    SELECT result INTO STRICT r FROM sdlc_pm_executions WHERE execution_id=NEW.execution_id AND task_id=t.task_id;
    SELECT * INTO STRICT root FROM sdlc_pm_resume_inputs WHERE task_id=t.task_id
        AND actor_subject=NEW.actor_subject AND idempotency_key=anchor->>'original_key';
    SELECT count(*) INTO attempts FROM sdlc_pm_lease_operations WHERE execution_id=NEW.execution_id
        AND recovery->>'kind'='settled_input_expiry'
        AND recovery#>>'{command,recovery,original_key}'=root.idempotency_key
        AND recovery#>>'{command,recovery,original_request_sha256}'=root.payload_hash;
    IF attempts=0 THEN
        predecessor_key:=root.idempotency_key; predecessor_hash:=root.payload_hash; predecessor:=root.result;
    ELSE
        SELECT idempotency_key,payload_hash,result INTO STRICT predecessor_key,predecessor_hash,predecessor
            FROM sdlc_pm_lease_operations WHERE execution_id=NEW.execution_id
            AND recovery->>'kind'='settled_input_expiry'
            AND recovery#>>'{command,recovery,original_key}'=root.idempotency_key
            AND recovery#>>'{command,recovery,original_request_sha256}'=root.payload_hash
            ORDER BY lease_version DESC LIMIT 1;
    END IF;
    IF (NEW.recovery ?& ARRAY['kind','command','witness','before','transaction_id']
        AND NEW.recovery-ARRAY['kind','command','witness','before','transaction_id']='{}'::jsonb
        AND NEW.recovery->>'kind'='settled_input_expiry'
        AND NEW.recovery->>'transaction_id'=pg_current_xact_id()::text
        AND NEW.recovery->'before'=l.result
        AND c ?& ARRAY['expected_owner_version','lease_id','expected_lease_version','input','checkpoint',
            'previous_run_id','previous_run_terminal','idempotency_key','recovery']
        AND c-ARRAY['expected_owner_version','lease_id','expected_lease_version','input','checkpoint',
            'previous_run_id','previous_run_terminal','idempotency_key','recovery']='{}'::jsonb
        AND anchor ?& ARRAY['kind','original_key','original_request_sha256','predecessor_key','predecessor_request_sha256','barrier_id']
        AND anchor-ARRAY['kind','original_key','original_request_sha256','predecessor_key','predecessor_request_sha256','barrier_id']='{}'::jsonb
        AND anchor->>'kind'='settled_input_expiry'
        AND (anchor->>'barrier_id')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
        AND anchor->>'original_key'=root.idempotency_key AND anchor->>'original_request_sha256'=root.payload_hash
        AND anchor->>'predecessor_key'=predecessor_key AND anchor->>'predecessor_request_sha256'=predecessor_hash
        AND c->>'idempotency_key'=NEW.idempotency_key AND NEW.idempotency_key<>root.idempotency_key
        AND NEW.idempotency_key<>predecessor_key AND attempts<3
        AND NOT root.command ? 'recovery' AND root.execution_id=NEW.execution_id
        AND c-ARRAY['lease_id','expected_lease_version','idempotency_key','recovery']=
            root.command-ARRAY['lease_id','expected_lease_version','idempotency_key']
        AND c->'lease_id'=root.result#>'{lease,lease_id}'
        AND t.task_id=t.root_task_id AND t.pm_execution_id=NEW.execution_id AND t.pm_admission_state='reserved'
        AND t.pm_owner_version=(c->>'expected_owner_version')::bigint
        AND r->'assignment'=t.state->'assignment' AND r->'binding'=NEW.result->'binding'
        AND r#>>'{binding,owner_subject}'=t.owner_subject AND r#>>'{binding,project_id}'=t.project_id::text
        AND r#>>'{binding,tracker_instance_id}'=t.tracker_instance_id
        AND r#>>'{binding,task_id}'=t.task_id::text AND r#>>'{binding,root_task_id}'=t.task_id::text
        AND r#>'{owner_cas,version}'=c->'expected_owner_version'
        AND c#>'{checkpoint,fence}'=l.result->'fence'
        AND c#>'{checkpoint,requirement_revision}'=COALESCE(t.state->'revisions'->-1->'revision','null'::jsonb)
        AND c#>'{checkpoint,content_hash}'=COALESCE(t.state->'revisions'->-1->'content_hash','null'::jsonb)
        AND ((root.input_kind IN ('feedback','answer') AND t.state->>'stage' IN ('Draft','Clarification'))
            OR (root.input_kind='confirmation' AND t.state->>'stage' IN ('Backlog','Analysis')))
        AND l.lease_id::text=c->>'lease_id' AND l.version=(c->>'expected_lease_version')::bigint
        AND l.version<9007199254740991 AND l.expires_at<=clock_timestamp()
        AND l.lease_id::text=predecessor#>>'{lease,lease_id}'
        AND l.holder_subject=predecessor#>>'{lease,holder_subject}'
        AND l.claimed_at=(predecessor#>>'{lease,claimed_at}')::timestamptz
        AND l.version>=(predecessor#>>'{lease,version}')::bigint
        AND l.heartbeat_at>=(predecessor#>>'{lease,heartbeat_at}')::timestamptz
        AND NEW.lease_version=l.version+1 AND NEW.result-'lease'=l.result-'lease'
        AND NEW.result#>>'{lease,holder_subject}'=l.holder_subject
        AND (NEW.result#>>'{lease,claimed_at}')::timestamptz=l.claimed_at
        AND (NEW.result#>>'{lease,heartbeat_at}')::timestamptz BETWEEN transaction_timestamp() AND clock_timestamp()
        AND (NEW.result#>>'{lease,expires_at}')::timestamptz=(NEW.result#>>'{lease,heartbeat_at}')::timestamptz+interval '30 seconds'
        AND NEW.result->'dispatch_allowed'='false'::jsonb
        AND w ?& ARRAY['contract_version','session_run_id','barrier_id','identity','request_sha256','command']
        AND w-ARRAY['contract_version','session_run_id','barrier_id','identity','request_sha256','command']='{}'::jsonb
        AND w->'contract_version'='1'::jsonb AND w->'session_run_id'=c->'previous_run_id'
        AND w->'barrier_id'=anchor->'barrier_id' AND w->>'request_sha256'=NEW.payload_hash AND w->'command'=c
        AND w->'identity'=jsonb_build_object('task',r#>'{execution,key}','execution_ref',r#>'{assignment,execution_id}',
            'tracker_instance_ref',r#>'{binding,tracker_instance_id}','tracker_project_ref',r#>'{binding,project_id}',
            'task_ref',r#>'{binding,task_id}','root_ref',r#>'{binding,root_task_id}','agent_ref',r#>'{assignment,agent_id}',
            'assignment_ref',r#>'{assignment,assignment_id}','assignment_revision',r#>'{assignment,version}',
            'assignment_operation_key',r->'assignment_operation_key')
    ) IS NOT TRUE
        OR EXISTS(SELECT 1 FROM sdlc_assignments a WHERE a.task_id=t.task_id
            AND a.version>(r#>>'{assignment,version}')::bigint)
        OR EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs WHERE execution_id=NEW.execution_id AND lease_version>root.lease_version)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=t.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=t.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE task_id=t.task_id)
        OR NOT EXISTS(SELECT 1 FROM users u JOIN projects p ON p.id=t.project_id WHERE u.central_sub=l.holder_subject
            AND u.is_active AND (p.owner_id=u.id OR EXISTS(SELECT 1 FROM project_members m WHERE m.project_id=p.id AND m.user_id=u.id))) THEN
        RAISE EXCEPTION 'Settled PM recovery requires original consumed input, latest predecessor, Fleet barrier and expired CAS' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_pm_settled_input_recovery_gate BEFORE INSERT ON sdlc_pm_lease_operations
FOR EACH ROW WHEN (NEW.recovery->>'kind'='settled_input_expiry')
EXECUTE FUNCTION sdlc_pm_settled_input_recovery_gate();
-- Existing expired-lease update and deferred atomic-renewal triggers are reused unchanged.
