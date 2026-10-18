-- Retain claim/heartbeat rows and use their existing immutable operation ledger.
ALTER TABLE sdlc_pm_lease_operations ADD COLUMN recovery jsonb;
ALTER TABLE sdlc_pm_lease_operations DROP CONSTRAINT sdlc_pm_lease_operations_check;
ALTER TABLE sdlc_pm_lease_operations ADD CONSTRAINT sdlc_pm_lease_operations_check CHECK (
    (result->'fence'->>'execution_id'=execution_id::text
    AND (result->'lease'->>'version')::bigint=lease_version
    AND (result->'lease'->>'holder_subject'=actor_subject OR recovery IS NOT NULL)
    AND result->'dispatch_allowed'='false'::jsonb) IS TRUE
);

CREATE FUNCTION sdlc_pm_unsubmitted_recovery_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks%ROWTYPE; l sdlc_pm_execution_leases%ROWTYPE; c jsonb; w jsonb; r jsonb;
BEGIN
    c:=NEW.recovery->'command'; w:=NEW.recovery->'witness';
    SELECT * INTO t FROM sdlc_tasks WHERE task_id=(NEW.result->'binding'->>'task_id')::uuid FOR UPDATE;
    SELECT * INTO l FROM sdlc_pm_execution_leases WHERE execution_id=NEW.execution_id FOR UPDATE;
    SELECT result INTO r FROM sdlc_pm_executions WHERE execution_id=NEW.execution_id AND task_id=t.task_id;
    IF (NEW.recovery ?& ARRAY['command','witness','transaction_id']
        AND NEW.recovery-ARRAY['command','witness','transaction_id']='{}'::jsonb
        AND NEW.recovery->>'transaction_id'=pg_current_xact_id()::text
        AND c ?& ARRAY['expected_owner_version','fence','lease_id','expected_lease_version','session_run_id','barrier_id','idempotency_key']
        AND c-ARRAY['expected_owner_version','fence','lease_id','expected_lease_version','session_run_id','barrier_id','idempotency_key']='{}'::jsonb
        AND c->>'idempotency_key'=NEW.idempotency_key
        AND t.task_id=t.root_task_id AND t.pm_admission_state='reserved' AND t.pm_execution_id=NEW.execution_id
        AND t.pm_owner_version=(c->>'expected_owner_version')::bigint
        AND t.state->>'flow'='single-task-v1' AND t.state->>'stage'='Draft'
        AND (t.state->>'status') IS DISTINCT FROM 'Completed'
        AND t.state->'revisions'='[]'::jsonb AND t.state->'questions'='[]'::jsonb AND t.state->'confirmations'='[]'::jsonb
        AND r->'owner_cas'->'expected_version'='0'::jsonb AND r->'assignment'->'version'='1'::jsonb
        AND r->'assignment'=t.state->'assignment' AND r->'binding'=NEW.result->'binding'
        AND r->'binding'->>'owner_subject'=t.owner_subject AND r->'binding'->>'project_id'=t.project_id::text
        AND r->'binding'->>'tracker_instance_id'=t.tracker_instance_id
        AND r->'binding'->>'task_id'=t.task_id::text AND r->'binding'->>'root_task_id'=t.task_id::text
        AND r->'owner_cas'->'version'=c->'expected_owner_version'
        AND c->'fence'=l.result->'fence' AND l.lease_id::text=c->>'lease_id'
        AND l.version=(c->>'expected_lease_version')::bigint AND l.expires_at<=clock_timestamp()
        AND NEW.lease_version=l.version+1 AND NEW.result-'lease'=l.result-'lease'
        AND NEW.result->'lease'->>'holder_subject'=l.holder_subject
        AND (NEW.result->'lease'->>'claimed_at')::timestamptz=l.claimed_at
        AND (NEW.result->'lease'->>'heartbeat_at')::timestamptz BETWEEN transaction_timestamp() AND clock_timestamp()
        AND (NEW.result->'lease'->>'expires_at')::timestamptz=(NEW.result->'lease'->>'heartbeat_at')::timestamptz+interval '30 seconds'
        AND c->>'session_run_id'<>'00000000-0000-0000-0000-000000000000'
        AND c->>'barrier_id'<>'00000000-0000-0000-0000-000000000000'
        AND w=jsonb_build_object('contract_version',1,'session_run_id',c->'session_run_id','barrier_id',c->'barrier_id',
            'lease_id',c->'lease_id','lease_version',c->'expected_lease_version','owner_version',c->'expected_owner_version',
            'identity',jsonb_build_object('task',r->'execution'->'key','execution_ref',r->'assignment'->'execution_id',
                'tracker_instance_ref',r->'binding'->'tracker_instance_id','tracker_project_ref',r->'binding'->'project_id',
                'task_ref',r->'binding'->'task_id','root_ref',r->'binding'->'root_task_id',
                'agent_ref',r->'assignment'->'agent_id','assignment_ref',r->'assignment'->'assignment_id',
                'assignment_revision',r->'assignment'->'version','assignment_operation_key',r->'assignment_operation_key'))
    ) IS NOT TRUE
        OR EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=t.task_id AND version>(r->'assignment'->>'version')::bigint)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=t.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=t.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE task_id=t.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_native_admissions WHERE execution_id=NEW.execution_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs WHERE execution_id=NEW.execution_id)
        OR NOT EXISTS(SELECT 1 FROM users u JOIN projects p ON p.id=t.project_id WHERE u.central_sub=l.holder_subject
            AND u.is_active AND (p.owner_id=u.id OR EXISTS(SELECT 1 FROM project_members m WHERE m.project_id=p.id AND m.user_id=u.id))) THEN
        RAISE EXCEPTION 'SDLC unsubmitted recovery requires exact current reserved identity and trusted barrier evidence' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_pm_unsubmitted_recovery_gate BEFORE INSERT ON sdlc_pm_lease_operations
FOR EACH ROW WHEN (NEW.recovery IS NOT NULL) EXECUTE FUNCTION sdlc_pm_unsubmitted_recovery_gate();

CREATE OR REPLACE FUNCTION sdlc_pm_lease_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'SDLC ownership lease cannot be deleted/reacquired' USING ERRCODE='23514';
    END IF;
    IF TG_OP='INSERT' THEN
        IF NEW.version!=1 THEN RAISE EXCEPTION 'SDLC initial ownership lease version invalid' USING ERRCODE='23514'; END IF;
        RETURN NEW;
    END IF;
    IF NEW.execution_id IS DISTINCT FROM OLD.execution_id OR NEW.lease_id IS DISTINCT FROM OLD.lease_id
        OR NEW.holder_subject IS DISTINCT FROM OLD.holder_subject OR NEW.claimed_at IS DISTINCT FROM OLD.claimed_at
        OR NEW.version!=OLD.version+1 OR NEW.heartbeat_at<OLD.heartbeat_at
        OR (NEW.result-'lease') IS DISTINCT FROM (OLD.result-'lease') THEN
        RAISE EXCEPTION 'SDLC inconsistent ownership lease CAS' USING ERRCODE='23514';
    END IF;
    IF OLD.expires_at<=clock_timestamp() AND NOT EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs r
        WHERE r.execution_id=OLD.execution_id AND r.lease_version=NEW.version
            AND r.transaction_id=pg_current_xact_id() AND r.result=NEW.result
            AND r.command->'lease_id'=to_jsonb(OLD.lease_id::text)
            AND r.command->'expected_lease_version'=to_jsonb(OLD.version))
        AND NOT EXISTS(SELECT 1 FROM sdlc_pm_lease_operations r
            WHERE r.execution_id=OLD.execution_id AND r.lease_version=NEW.version
                AND r.recovery->>'transaction_id'=pg_current_xact_id()::text AND r.result=NEW.result
                AND r.recovery->'command'->'lease_id'=to_jsonb(OLD.lease_id::text)
                AND r.recovery->'command'->'expected_lease_version'=to_jsonb(OLD.version)) THEN
        RAISE EXCEPTION 'SDLC expired ownership lease requires exact trusted resume transaction' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

-- A recovery evidence insert alone must not commit as a false renewal receipt.
CREATE FUNCTION sdlc_pm_unsubmitted_recovery_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_pm_execution_leases l WHERE l.execution_id=NEW.execution_id
        AND l.version>=NEW.lease_version AND (l.version<>NEW.lease_version OR l.result=NEW.result)) THEN
        RAISE EXCEPTION 'SDLC unsubmitted recovery evidence requires atomic lease renewal' USING ERRCODE='23514';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_pm_unsubmitted_recovery_commit_gate AFTER INSERT ON sdlc_pm_lease_operations
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN (NEW.recovery IS NOT NULL)
EXECUTE FUNCTION sdlc_pm_unsubmitted_recovery_commit_gate();
