-- Extend the existing immutable lease-operation ledger. No claim/history reset.
-- 0098's expired lease and deferred atomic-renewal gates remain unchanged.
DROP TRIGGER sdlc_pm_unsubmitted_recovery_gate ON sdlc_pm_lease_operations;
CREATE TRIGGER sdlc_pm_unsubmitted_recovery_gate BEFORE INSERT ON sdlc_pm_lease_operations
FOR EACH ROW WHEN (NEW.recovery IS NOT NULL AND NEW.recovery->>'kind' IS DISTINCT FROM 'bootstrap_retry')
EXECUTE FUNCTION sdlc_pm_unsubmitted_recovery_gate();

-- This first-attempt-only path consumes the original predecessor once, regardless
-- of actor/key. Original-key replay reads history instead of inserting a new row.
CREATE UNIQUE INDEX sdlc_pm_bootstrap_retry_original ON sdlc_pm_lease_operations(execution_id)
WHERE recovery->>'kind'='bootstrap_retry';
CREATE UNIQUE INDEX sdlc_pm_bootstrap_retry_successor ON sdlc_pm_lease_operations((recovery->'command'->>'next_session_run_id'))
WHERE recovery->>'kind'='bootstrap_retry';

CREATE FUNCTION sdlc_pm_bootstrap_retry_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks%ROWTYPE; l sdlc_pm_execution_leases%ROWTYPE; c jsonb; w jsonb; r jsonb; p jsonb;
BEGIN
    c:=NEW.recovery->'command'; w:=NEW.recovery->'witness'; p:=c->'predecessor';
    SELECT * INTO t FROM sdlc_tasks WHERE task_id=(NEW.result->'binding'->>'task_id')::uuid FOR UPDATE;
    SELECT * INTO l FROM sdlc_pm_execution_leases WHERE execution_id=NEW.execution_id FOR UPDATE;
    SELECT result INTO r FROM sdlc_pm_executions WHERE execution_id=NEW.execution_id AND task_id=t.task_id;
    IF (NEW.recovery ?& ARRAY['kind','command','witness','before','reservation','transaction_id']
        AND NEW.recovery-ARRAY['kind','command','witness','before','reservation','transaction_id']='{}'::jsonb
        AND NEW.recovery->>'kind'='bootstrap_retry'
        AND NEW.recovery->>'transaction_id'=pg_current_xact_id()::text
        AND NEW.recovery->'before'=l.result AND NEW.recovery->'reservation'=r
        AND c ?& ARRAY['expected_owner_version','fence','lease_id','expected_lease_version','creation_operation_id',
            'previous_session_run_id','next_session_run_id','previous_native_run_ref','chat_id','barrier_id','predecessor','idempotency_key']
        AND c-ARRAY['expected_owner_version','fence','lease_id','expected_lease_version','creation_operation_id',
            'previous_session_run_id','next_session_run_id','previous_native_run_ref','chat_id','barrier_id','predecessor','idempotency_key']='{}'::jsonb
        AND c->>'idempotency_key'=NEW.idempotency_key
        AND t.task_id=t.root_task_id AND t.pm_admission_state='reserved' AND t.pm_execution_id=NEW.execution_id
        AND t.pm_owner_version=1 AND c->'expected_owner_version'='1'::jsonb
        AND t.state->>'flow'='single-task-v1' AND t.state->>'stage'='Draft'
        AND t.state->>'status'='InProgress' AND t.state->>'draft_submode'='brainstorming'
        AND t.state->'revisions'='[]'::jsonb AND t.state->'questions'='[]'::jsonb
        AND t.state->'confirmations'='[]'::jsonb AND t.state->'evidence'='[]'::jsonb
        AND t.state->'confirmation_revision'='null'::jsonb
        AND r->'owner_cas'->'expected_version'='0'::jsonb AND r->'owner_cas'->'version'='1'::jsonb
        AND r->'assignment'->'version'='1'::jsonb AND r->'assignment'=t.state->'assignment'
        AND r->'binding'=NEW.result->'binding' AND r->'binding'->>'owner_subject'=t.owner_subject
        AND r->'binding'->>'project_id'=t.project_id::text AND r->'binding'->>'tracker_instance_id'=t.tracker_instance_id
        AND r->'binding'->>'task_id'=t.task_id::text AND r->'binding'->>'root_task_id'=t.task_id::text
        AND c->'fence'=l.result->'fence' AND l.lease_id::text=c->>'lease_id'
        AND l.version=(c->>'expected_lease_version')::bigint AND l.version<9007199254740991
        AND l.expires_at<=clock_timestamp() AND NEW.lease_version=l.version+1
        AND NEW.result-'lease'=l.result-'lease'
        AND NEW.result->'lease'->>'holder_subject'=l.holder_subject
        AND NEW.actor_subject<>l.holder_subject AND NEW.actor_subject<>t.owner_subject
        AND (NEW.result->'lease'->>'claimed_at')::timestamptz=l.claimed_at
        AND (NEW.result->'lease'->>'heartbeat_at')::timestamptz BETWEEN transaction_timestamp() AND clock_timestamp()
        AND (NEW.result->'lease'->>'expires_at')::timestamptz=(NEW.result->'lease'->>'heartbeat_at')::timestamptz+interval '30 seconds'
        AND c->>'creation_operation_id'=c->>'previous_session_run_id'
        AND (c->>'previous_session_run_id')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
        AND (c->>'next_session_run_id')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
        AND c->>'next_session_run_id'<>c->>'previous_session_run_id'
        AND (c->>'chat_id')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
        AND (c->>'barrier_id')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
        AND length(c->>'previous_native_run_ref') BETWEEN 1 AND 256
        AND p ?& ARRAY['task_id','assignment_operation_key','assignment_revision','attempt','finalized_binding']
        AND p-ARRAY['task_id','assignment_operation_key','assignment_revision','attempt','finalized_binding']='{}'::jsonb
        AND (p->>'task_id')::bigint BETWEEN 1 AND 9007199254740990
        AND p->'assignment_revision'='1'::jsonb AND p->'attempt'='1'::jsonb
        AND p->>'assignment_operation_key'=r->>'assignment_operation_key'
        AND (p->'finalized_binding'='null'::jsonb OR (
            p->'finalized_binding' ?& ARRAY['bind_operation_key','bind_request_sha256','binding_ref','session_run_id','native_run_ref']
            AND (p->'finalized_binding')-ARRAY['bind_operation_key','bind_request_sha256','binding_ref','session_run_id','native_run_ref']='{}'::jsonb
            AND length(p->'finalized_binding'->>'bind_operation_key') BETWEEN 1 AND 128
            AND p->'finalized_binding'->>'bind_request_sha256' ~ '^[0-9a-f]{64}$'
            AND length(p->'finalized_binding'->>'binding_ref') BETWEEN 1 AND 512
            AND p->'finalized_binding'->>'session_run_id'=c->>'previous_session_run_id'
            AND p->'finalized_binding'->>'native_run_ref'=c->>'previous_native_run_ref'))
        AND w ?& ARRAY['contract_version','observed_at','identity','owner_subject','input','owner_version','lease_id','lease_version',
            'creation_operation_id','session_run_id','native_run_ref','chat_id','barrier_id','next_session_run_id','terminal','predecessor',
            'initial_phase_id','current_phase_id','workflow_status','first_instruction','pm_enrollment','generic_binding','business_result',
            'inflight_work','terminal_receipt','bootstrap_receipt']
        AND w-ARRAY['contract_version','observed_at','identity','owner_subject','input','owner_version','lease_id','lease_version',
            'creation_operation_id','session_run_id','native_run_ref','chat_id','barrier_id','next_session_run_id','terminal','predecessor',
            'initial_phase_id','current_phase_id','workflow_status','first_instruction','pm_enrollment','generic_binding','business_result',
            'inflight_work','terminal_receipt','bootstrap_receipt']='{}'::jsonb
        AND w->'contract_version'='1'::jsonb AND w->'owner_version'=c->'expected_owner_version'
        AND w->'lease_id'=c->'lease_id' AND w->'lease_version'=c->'expected_lease_version'
        AND w->'creation_operation_id'=c->'creation_operation_id' AND w->'session_run_id'=c->'previous_session_run_id'
        AND w->'native_run_ref'=c->'previous_native_run_ref' AND w->'chat_id'=c->'chat_id'
        AND w->'barrier_id'=c->'barrier_id' AND w->'next_session_run_id'=c->'next_session_run_id'
        AND w->'predecessor'=p AND w->>'terminal' IN ('completed','failed','cancelled','stopped')
        AND w->>'first_instruction'='absent' AND w->>'pm_enrollment'='absent'
        AND w->>'business_result'='absent' AND w->>'inflight_work'='absent'
        AND w->>'generic_binding'=CASE WHEN p->'finalized_binding'='null'::jsonb THEN 'absent' ELSE 'present' END
        AND (w->>'initial_phase_id')::bigint BETWEEN 1 AND 9007199254740991
        AND w->'current_phase_id'=w->'initial_phase_id' AND w->>'workflow_status'='active'
        AND (w->>'observed_at')::timestamptz BETWEEN (NEW.result->'lease'->>'heartbeat_at')::timestamptz-interval '30 seconds'
            AND (NEW.result->'lease'->>'heartbeat_at')::timestamptz
        AND w->'input'=r->'input' AND w->>'owner_subject'=t.owner_subject
        AND w->'identity'=jsonb_build_object('task',r->'execution'->'key','execution_ref',r->'assignment'->'execution_id',
            'tracker_instance_ref',r->'binding'->'tracker_instance_id','tracker_project_ref',r->'binding'->'project_id',
            'task_ref',r->'binding'->'task_id','root_ref',r->'binding'->'root_task_id','agent_ref',r->'assignment'->'agent_id',
            'assignment_ref',r->'assignment'->'assignment_id','assignment_revision',r->'assignment'->'version',
            'assignment_operation_key',r->'assignment_operation_key')
        AND (w->'terminal_receipt')-ARRAY['reference','sha256']='{}'::jsonb
        AND (w->'bootstrap_receipt')-ARRAY['reference','sha256']='{}'::jsonb
        AND length(w->'terminal_receipt'->>'reference') BETWEEN 1 AND 512
        AND length(w->'bootstrap_receipt'->>'reference') BETWEEN 1 AND 512
        AND w->'terminal_receipt'->>'sha256' ~ '^[0-9a-f]{64}$'
        AND w->'bootstrap_receipt'->>'sha256' ~ '^[0-9a-f]{64}$'
    ) IS NOT TRUE
        OR EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=t.task_id AND version>1)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=t.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=t.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE task_id=t.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_native_admissions WHERE execution_id=NEW.execution_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs WHERE execution_id=NEW.execution_id)
        OR NOT EXISTS(SELECT 1 FROM users u JOIN projects p ON p.id=t.project_id WHERE u.central_sub=l.holder_subject
            AND u.is_active AND (p.owner_id=u.id OR EXISTS(SELECT 1 FROM project_members m WHERE m.project_id=p.id AND m.user_id=u.id))) THEN
        RAISE EXCEPTION 'SDLC bootstrap retry requires exact terminal pre-enrollment proof and expired lease' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_pm_bootstrap_retry_gate BEFORE INSERT ON sdlc_pm_lease_operations
FOR EACH ROW WHEN (NEW.recovery->>'kind'='bootstrap_retry') EXECUTE FUNCTION sdlc_pm_bootstrap_retry_gate();
