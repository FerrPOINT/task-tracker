use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP_SQL: &str = r#"
ALTER TABLE sdlc_tasks DROP CONSTRAINT IF EXISTS sdlc_pm_control;
ALTER TABLE sdlc_tasks ADD CONSTRAINT sdlc_pm_control CHECK (
    (pm_owner_version=0 AND pm_execution_id IS NULL AND pm_admission_state IS NULL)
    OR (pm_owner_version=1 AND pm_execution_id IS NOT NULL
        AND pm_admission_state IN ('reserved','completed')
        AND (state->'assignment'->>'execution_id'=pm_execution_id::text) IS TRUE)
);

CREATE TABLE sdlc_pm_completions (
    completion_id uuid PRIMARY KEY CHECK(completion_id!='00000000-0000-0000-0000-000000000000'::uuid),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    execution_id uuid NOT NULL UNIQUE REFERENCES sdlc_pm_executions(execution_id) ON DELETE RESTRICT,
    assignment_id uuid NOT NULL CHECK(assignment_id!='00000000-0000-0000-0000-000000000000'::uuid),
    assignment_version bigint NOT NULL CHECK(assignment_version BETWEEN 1 AND 9007199254740991),
    lease_id uuid NOT NULL CHECK(lease_id!='00000000-0000-0000-0000-000000000000'::uuid),
    lease_version bigint NOT NULL CHECK(lease_version BETWEEN 1 AND 9007199254740991),
    requirement_revision bigint NOT NULL CHECK(requirement_revision BETWEEN 1 AND 9007199254740991),
    content_hash text NOT NULL CHECK(content_hash ~ '^[0-9a-f]{64}$'),
    terminal_receipt_ref text NOT NULL CHECK(octet_length(terminal_receipt_ref) BETWEEN 1 AND 512),
    terminal_receipt_sha256 text NOT NULL CHECK(terminal_receipt_sha256 ~ '^[0-9a-f]{64}$'),
    workflow_receipt_ref text NOT NULL CHECK(octet_length(workflow_receipt_ref) BETWEEN 1 AND 512),
    workflow_receipt_sha256 text NOT NULL CHECK(workflow_receipt_sha256 ~ '^[0-9a-f]{64}$'),
    result jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(task_id,execution_id),
    CHECK((result->'contract_version'='1'::jsonb
        AND result->>'completion_id'=completion_id::text
        AND result->>'task_id'=task_id::text
        AND result->'fence'->>'execution_id'=execution_id::text
        AND result->'fence'->>'assignment_id'=assignment_id::text
        AND result->'fence'->>'assignment_version'=assignment_version::text
        AND result->>'lease_id'=lease_id::text
        AND result->'lease_version'=to_jsonb(lease_version)
        AND result->'requirement_revision'=to_jsonb(requirement_revision)
        AND result->>'content_hash'=content_hash
        AND result->>'terminal_receipt_ref'=terminal_receipt_ref
        AND result->>'terminal_receipt_sha256'=terminal_receipt_sha256
        AND result->>'workflow_receipt_ref'=workflow_receipt_ref
        AND result->>'workflow_receipt_sha256'=workflow_receipt_sha256
        AND result->>'previous_pm_admission_state'='reserved'
        AND result->>'next_pm_admission_state'='completed'
        AND (result->>'owner_version')::bigint BETWEEN 1 AND 9007199254740991
        AND octet_length(result->>'idempotency_key') BETWEEN 1 AND 128) IS TRUE)
);

CREATE TABLE sdlc_pm_completion_operations (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    payload_hash text NOT NULL CHECK(payload_hash ~ '^[0-9a-f]{64}$'),
    completion_id uuid NOT NULL UNIQUE REFERENCES sdlc_pm_completions(completion_id) ON DELETE RESTRICT,
    result jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(task_id,idempotency_key),
    CHECK((result->>'task_id'=task_id::text
        AND result->>'completion_id'=completion_id::text
        AND result->>'idempotency_key'=idempotency_key
        AND result->>'next_pm_admission_state'='completed') IS TRUE)
);

CREATE OR REPLACE FUNCTION sdlc_current_pm_completed(
    task uuid,
    execution uuid,
    state jsonb,
    admission_state text
) RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT admission_state='completed'
        AND execution IS NOT NULL
        AND EXISTS(
            SELECT 1 FROM sdlc_pm_completions c
            WHERE c.task_id=task
                AND c.execution_id=execution
                AND c.assignment_id::text=(state #>> '{assignment,assignment_id}')
                AND c.assignment_version::text=(state #>> '{assignment,version}')
                AND c.requirement_revision::text=(state->'revisions'->-1->>'revision')
                AND c.content_hash=(state->'revisions'->-1->>'content_hash')
                AND c.result->>'next_pm_admission_state'='completed')
$$;

CREATE OR REPLACE FUNCTION sdlc_current_pm_draft_reserved(
    task uuid,
    execution uuid,
    state jsonb,
    owner_version bigint,
    admission_state text
) RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT admission_state='reserved'
        AND execution IS NOT NULL
        AND EXISTS(
            SELECT 1 FROM sdlc_pm_executions e
            WHERE e.task_id=task
                AND e.execution_id=execution
                AND e.assignment_id::text=(state #>> '{assignment,assignment_id}')
                AND e.assignment_version::text=(state #>> '{assignment,version}')
                AND e.owner_version=owner_version
                AND e.result->>'variant'='pm_draft_reserved'
                AND e.result->>'admission_state'='reserved'
                AND e.result->'dispatch_allowed'='false'::jsonb)
$$;

CREATE FUNCTION sdlc_pm_completion_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t
        JOIN sdlc_pm_execution_leases l ON l.execution_id=NEW.execution_id
        WHERE t.task_id=NEW.task_id
            AND t.pm_execution_id=NEW.execution_id
            AND t.pm_admission_state='reserved'
            AND t.pm_owner_version=(NEW.result->>'owner_version')::bigint
            AND l.lease_id=NEW.lease_id
            AND l.version=NEW.lease_version
            AND t.state->'assignment'->>'assignment_id'=NEW.assignment_id::text
            AND t.state->'assignment'->>'execution_id'=NEW.execution_id::text
            AND t.state->'assignment'->>'version'=NEW.assignment_version::text
            AND t.state->'assignment'->>'agent_id'=NEW.result->'fence'->>'agent_id'
            AND t.state->'revisions'->-1->>'revision'=NEW.requirement_revision::text
            AND t.state->'revisions'->-1->>'content_hash'=NEW.content_hash
            AND NEW.result->>'tracker_instance_id'=t.tracker_instance_id
            AND NEW.result->>'project_id'=t.project_id::text
            AND NEW.result->>'root_task_id'=t.root_task_id::text
            AND NEW.result->>'owner_subject'=t.owner_subject) THEN
        RAISE EXCEPTION 'SDLC PM completion requires exact reserved execution lease'
            USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_pm_completion_gate BEFORE INSERT ON sdlc_pm_completions
FOR EACH ROW EXECUTE FUNCTION sdlc_pm_completion_gate();

CREATE OR REPLACE FUNCTION sdlc_pm_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF OLD.pm_execution_id IS NOT NULL THEN
    IF NEW.pm_owner_version IS DISTINCT FROM OLD.pm_owner_version
      OR NEW.pm_execution_id IS DISTINCT FROM OLD.pm_execution_id
      OR NEW.state->'assignment' IS DISTINCT FROM OLD.state->'assignment'
      OR NEW.state->'task_id' IS DISTINCT FROM OLD.state->'task_id'
      OR NEW.state->'root_task_id' IS DISTINCT FROM OLD.state->'root_task_id'
      OR NEW.state->'project_id' IS DISTINCT FROM OLD.state->'project_id'
      OR NEW.state->'tracker_instance_id' IS DISTINCT FROM OLD.state->'tracker_instance_id'
      OR NEW.state->'owner_subject' IS DISTINCT FROM OLD.state->'owner_subject' THEN
      RAISE EXCEPTION 'SDLC reserved PM ownership is immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.pm_admission_state IS DISTINCT FROM OLD.pm_admission_state THEN
      IF NOT (OLD.pm_admission_state='reserved' AND NEW.pm_admission_state='completed'
        AND sdlc_current_pm_completed(OLD.task_id, OLD.pm_execution_id, OLD.state, 'completed')) THEN
        RAISE EXCEPTION 'SDLC reserved PM completion transition is not authorized'
          USING ERRCODE='23514';
      END IF;
    END IF;
    IF NEW.state IS DISTINCT FROM OLD.state THEN
      IF NOT (
        EXISTS(
          SELECT 1 FROM sdlc_pm_native_admissions a WHERE a.execution_id=OLD.pm_execution_id
            AND a.assignment_id::text=(OLD.state #>> '{assignment,assignment_id}')
            AND a.assignment_version::text=(OLD.state #>> '{assignment,version}')
            AND (a.proof #>> '{identity,task_ref}')=OLD.task_id::text)
        OR sdlc_current_pm_completed(OLD.task_id, OLD.pm_execution_id, OLD.state, OLD.pm_admission_state)
        OR sdlc_current_pm_draft_reserved(OLD.task_id, OLD.pm_execution_id, OLD.state,
            OLD.pm_owner_version, OLD.pm_admission_state)
      ) THEN
        RAISE EXCEPTION 'SDLC reserved PM requires verified reservation, completion or native admission'
          USING ERRCODE='23514';
      END IF;
      IF NOT EXISTS(
        SELECT 1 FROM sdlc_pm_state_write_permits p WHERE p.task_id=OLD.task_id
          AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN
        RAISE EXCEPTION 'SDLC reserved PM requires an authorized state write' USING ERRCODE='23514';
      END IF;
      DELETE FROM sdlc_pm_state_write_permits WHERE task_id=OLD.task_id
        AND transaction_id=pg_current_xact_id() AND next_state=NEW.state;
    END IF;
  END IF;
  RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION sdlc_analysis_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM task_id FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=NEW.root_task_id
        AND (EXISTS(SELECT 1 FROM sdlc_assignments a WHERE a.task_id=t.task_id)
            OR EXISTS(SELECT 1 FROM sdlc_pm_executions e WHERE e.task_id=t.task_id)
            OR t.pm_execution_id IS NOT NULL
            OR t.state->'assignment' IS DISTINCT FROM 'null'::jsonb)
        AND NOT sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state))
        OR EXISTS(SELECT 1 FROM sdlc_assignments a
            JOIN sdlc_tasks t ON t.task_id=a.task_id
                AND t.pm_execution_id=(a.payload->>'execution_id')::uuid
            WHERE a.payload->>'agent_id'=NEW.agent_id::text
                AND NOT sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)) THEN
        RAISE EXCEPTION 'SDLC pm_quiescence_unverified' USING ERRCODE='23514';
    END IF;
    IF EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=NEW.root_task_id OR agent_id=NEW.agent_id) THEN
        RAISE EXCEPTION 'SDLC root/agent capacity held by Analysis reservation' USING ERRCODE='23514';
    END IF;
    IF NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents i JOIN sdlc_tasks t USING(task_id)
        JOIN sdlc_task_routing_snapshots s ON s.snapshot_id=i.routing_snapshot_id
        WHERE i.intent_id=NEW.intent_id AND i.task_id=NEW.task_id AND s.snapshot_id=NEW.routing_snapshot_id
        AND t.state->>'stage'='Analysis' AND t.root_task_id=NEW.root_task_id
        AND i.requirement_revision=(t.state->'revisions'->-1->>'revision')::bigint
        AND i.content_hash=t.state->'revisions'->-1->>'content_hash'
        AND NEW.payload->>'tracker_instance_id'=t.tracker_instance_id
        AND NEW.payload->>'owner_subject'=t.owner_subject AND NEW.payload->>'project_id'=t.project_id::text
        AND NEW.payload->>'confirmation_id'=i.confirmation_id::text
        AND NEW.payload->'requirement_revision'=to_jsonb(i.requirement_revision)
        AND NEW.payload->>'content_hash'=i.content_hash
        AND NEW.payload->'routing_policy_version'=to_jsonb(s.policy_version)
        AND NEW.payload->>'routing_hash'=s.payload->'policy'->>'routing_hash'
        AND NEW.payload->'route'=s.payload->'policy'->'routes'->'analyst'
        AND NEW.agent_id::text=s.payload->'policy'->'routes'->'analyst'->>'agent_id') THEN
        RAISE EXCEPTION 'SDLC reservation requires exact routed current Analysis intent' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION sdlc_analysis_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state->>'stage'='Analysis' AND NEW.state IS DISTINCT FROM OLD.state THEN
        IF NEW.state->>'stage'!='Architecture' THEN
            RAISE EXCEPTION 'SDLC queued Analysis requires guarded admission before lifecycle mutation'
                USING ERRCODE='23514';
        END IF;
        IF NOT EXISTS(SELECT 1 FROM sdlc_analysis_state_write_permits p
            WHERE p.task_id=OLD.task_id AND p.transaction_id=pg_current_xact_id()
                AND p.next_state=NEW.state) THEN
            RAISE EXCEPTION 'SDLC Analysis result requires an authorized state write'
                USING ERRCODE='23514';
        END IF;
        IF NOT EXISTS(SELECT 1 FROM sdlc_analysis_results r
            WHERE r.task_id=OLD.task_id
                AND r.payload->>'requirement_revision'=OLD.state->'revisions'->-1->>'revision'
                AND r.payload->>'content_hash'=OLD.state->'revisions'->-1->>'content_hash') THEN
            RAISE EXCEPTION 'SDLC Analysis result history absent' USING ERRCODE='23514';
        END IF;
        DELETE FROM sdlc_analysis_state_write_permits
        WHERE task_id=OLD.task_id AND transaction_id=pg_current_xact_id()
            AND next_state=NEW.state;
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER sdlc_history_immutable BEFORE UPDATE OR DELETE ON sdlc_pm_completions
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
CREATE TRIGGER sdlc_history_immutable BEFORE UPDATE OR DELETE ON sdlc_pm_completion_operations
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP_SQL).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "PM completion history is additive; rollback requires preserving lifecycle history"
                .into(),
        ))
    }
}
