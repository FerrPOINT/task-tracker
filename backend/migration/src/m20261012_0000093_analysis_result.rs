use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP_SQL: &str = r#"
INSERT INTO statuses (name)
SELECT name
FROM (VALUES ('SDLC Architecture')) AS names(name)
WHERE NOT EXISTS (SELECT 1 FROM statuses s WHERE s.name = names.name);

ALTER TABLE sdlc_tasks DROP CONSTRAINT IF EXISTS sdlc_tasks_stage_known;
DO $$ DECLARE constraint_name text; BEGIN
    FOR constraint_name IN
        SELECT conname FROM pg_constraint
        WHERE conrelid='sdlc_tasks'::regclass AND contype='c'
            AND pg_get_constraintdef(oid) LIKE '%state%stage%'
    LOOP
        EXECUTE format('ALTER TABLE sdlc_tasks DROP CONSTRAINT %I', constraint_name);
    END LOOP;
END $$;
ALTER TABLE sdlc_tasks ADD CONSTRAINT sdlc_tasks_stage_known
    CHECK (state->>'stage' IN ('Draft', 'Clarification', 'Backlog', 'Analysis', 'Architecture'));

DO $$ DECLARE constraint_name text; BEGIN
    FOR constraint_name IN
        SELECT conname FROM pg_constraint
        WHERE conrelid='sdlc_analysis_reservations'::regclass AND contype='u'
            AND pg_get_constraintdef(oid) IN ('UNIQUE (root_task_id)', 'UNIQUE (agent_id)', 'UNIQUE (pool_slot)')
    LOOP
        EXECUTE format('ALTER TABLE sdlc_analysis_reservations DROP CONSTRAINT %I', constraint_name);
    END LOOP;
END $$;

CREATE TABLE sdlc_analysis_results (
    result_id uuid PRIMARY KEY CHECK(result_id!='00000000-0000-0000-0000-000000000000'::uuid),
    assignment_id uuid NOT NULL UNIQUE REFERENCES sdlc_analysis_reservations(assignment_id) ON DELETE RESTRICT,
    execution_id uuid NOT NULL UNIQUE CHECK(execution_id!='00000000-0000-0000-0000-000000000000'::uuid),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    intent_id uuid NOT NULL UNIQUE REFERENCES sdlc_analysis_intents(intent_id) ON DELETE RESTRICT,
    lease_version bigint NOT NULL CHECK(lease_version BETWEEN 1 AND 9007199254740991),
    terminal_receipt_ref text NOT NULL CHECK(octet_length(terminal_receipt_ref) BETWEEN 1 AND 512),
    terminal_receipt_sha256 text NOT NULL CHECK(terminal_receipt_sha256 ~ '^[0-9a-f]{64}$'),
    architecture_input_sha256 text NOT NULL CHECK(architecture_input_sha256 ~ '^[0-9a-f]{64}$'),
    payload jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK((payload->'contract_version'='1'::jsonb
        AND payload->>'result_id'=result_id::text
        AND payload->>'assignment_id'=assignment_id::text
        AND payload->>'execution_id'=execution_id::text
        AND payload->>'task_id'=task_id::text
        AND payload->>'intent_id'=intent_id::text
        AND payload->'lease_version'=to_jsonb(lease_version)
        AND payload->>'previous_stage'='Analysis'
        AND payload->>'next_stage'='Architecture'
        AND payload->>'previous_status'='SDLC Analysis'
        AND payload->>'next_status'='SDLC Architecture'
        AND payload->>'terminal_receipt_ref'=terminal_receipt_ref
        AND payload->>'terminal_receipt_sha256'=terminal_receipt_sha256
        AND payload->>'architecture_input_sha256'=architecture_input_sha256
        AND payload->>'content_hash' ~ '^[0-9a-f]{64}$'
        AND payload->'requirement_revision'=to_jsonb((payload->>'requirement_revision')::bigint)
        AND (payload->>'requirement_revision')::bigint BETWEEN 1 AND 9007199254740991
        AND char_length(payload->>'summary') BETWEEN 1 AND 100000
        AND octet_length(payload->>'idempotency_key') BETWEEN 1 AND 128) IS TRUE)
);

CREATE TABLE sdlc_analysis_result_operations (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    payload_hash text NOT NULL CHECK(payload_hash ~ '^[0-9a-f]{64}$'),
    result_id uuid NOT NULL UNIQUE REFERENCES sdlc_analysis_results(result_id) ON DELETE RESTRICT,
    result jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(task_id,idempotency_key),
    CHECK((result->>'task_id'=task_id::text
        AND result->>'result_id'=result_id::text
        AND result->>'idempotency_key'=idempotency_key
        AND result->>'next_stage'='Architecture') IS TRUE)
);

CREATE TABLE sdlc_analysis_state_write_permits (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    transaction_id xid8 NOT NULL,
    next_state jsonb NOT NULL,
    PRIMARY KEY(task_id,transaction_id)
);

CREATE TABLE sdlc_analysis_capacity_holds (
    assignment_id uuid PRIMARY KEY REFERENCES sdlc_analysis_reservations(assignment_id) ON DELETE RESTRICT,
    root_task_id uuid NOT NULL UNIQUE REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    agent_id uuid NOT NULL UNIQUE CHECK(agent_id!='00000000-0000-0000-0000-000000000000'::uuid),
    pool_slot smallint NOT NULL UNIQUE CHECK(pool_slot BETWEEN 1 AND 2),
    created_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO sdlc_analysis_capacity_holds(assignment_id,root_task_id,agent_id,pool_slot)
SELECT assignment_id,root_task_id,agent_id,pool_slot
FROM sdlc_analysis_reservations
ON CONFLICT DO NOTHING;

CREATE OR REPLACE FUNCTION sdlc_analysis_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM task_id FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=NEW.root_task_id OR payload->>'agent_id'=NEW.agent_id::text)
        OR EXISTS(SELECT 1 FROM sdlc_pm_executions WHERE task_id=NEW.root_task_id)
        OR EXISTS(SELECT 1 FROM sdlc_tasks WHERE task_id=NEW.root_task_id AND (pm_execution_id IS NOT NULL OR state->'assignment' IS DISTINCT FROM 'null'::jsonb)) THEN
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

CREATE OR REPLACE FUNCTION sdlc_pm_capacity_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM task_id FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=NEW.task_id OR agent_id::text=NEW.payload->>'agent_id') THEN
        RAISE EXCEPTION 'SDLC root/agent capacity held by Analysis reservation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE FUNCTION sdlc_analysis_result_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_analysis_reservations r
        JOIN sdlc_analysis_reservation_leases l USING(assignment_id)
        JOIN sdlc_analysis_capacity_holds h USING(assignment_id)
        JOIN sdlc_analysis_intents i ON i.intent_id=r.intent_id
        JOIN sdlc_tasks t ON t.task_id=r.task_id
        WHERE r.assignment_id=NEW.assignment_id AND r.execution_id=NEW.execution_id
            AND r.task_id=NEW.task_id AND r.intent_id=NEW.intent_id
            AND l.version=NEW.lease_version AND l.expires_at>clock_timestamp()
            AND t.state->>'stage'='Analysis'
            AND i.requirement_revision=(t.state->'revisions'->-1->>'revision')::bigint
            AND i.content_hash=t.state->'revisions'->-1->>'content_hash'
            AND NEW.payload->>'tracker_instance_id'=t.tracker_instance_id
            AND NEW.payload->>'project_id'=t.project_id::text
            AND NEW.payload->>'root_task_id'=t.root_task_id::text
            AND NEW.payload->>'owner_subject'=t.owner_subject
            AND NEW.payload->>'workflow_task_ref'=r.payload->>'workflow_task_ref'
            AND NEW.payload->>'agent_id'=r.agent_id::text
            AND NEW.payload->>'routing_snapshot_id'=r.routing_snapshot_id::text
            AND NEW.payload->>'fencing_token'=r.fencing_token::text
            AND NEW.payload->>'lease_id'=r.lease_id::text
            AND NEW.payload->>'requirement_revision'=i.requirement_revision::text
            AND NEW.payload->>'content_hash'=i.content_hash) THEN
        RAISE EXCEPTION 'SDLC Analysis result requires active exact reservation lease' USING ERRCODE='23514';
    END IF;
    DELETE FROM sdlc_analysis_capacity_holds WHERE assignment_id=NEW.assignment_id;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_analysis_result_gate BEFORE INSERT ON sdlc_analysis_results
FOR EACH ROW EXECUTE FUNCTION sdlc_analysis_result_gate();

CREATE OR REPLACE FUNCTION sdlc_analysis_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state->>'stage'='Analysis' AND NEW IS DISTINCT FROM OLD THEN
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

CREATE OR REPLACE FUNCTION sdlc_issue_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE binding sdlc_tasks%ROWTYPE; target_name text;
BEGIN
    SELECT * INTO binding FROM sdlc_tasks WHERE task_id = NEW.id;
    IF NOT FOUND THEN RETURN NEW; END IF;
    IF NEW.project_id != binding.project_id THEN
        RAISE EXCEPTION 'SDLC project binding is immutable' USING ERRCODE = '23514';
    END IF;
    SELECT name INTO target_name FROM statuses WHERE id = NEW.status_id;
    IF binding.state->>'stage' IN ('Draft', 'Clarification') THEN
        IF target_name != 'SDLC ' || (binding.state->>'stage') OR NEW.sprint_id IS NOT NULL THEN
            RAISE EXCEPTION 'SDLC confirmation gate blocks transition/sprint' USING ERRCODE = '23514';
        END IF;
    ELSIF NOT EXISTS (
        SELECT 1 FROM sdlc_confirmations c WHERE c.task_id = NEW.id
        AND c.revision = (binding.state->'revisions'->-1->>'revision')::bigint
        AND c.content_hash = binding.state->'revisions'->-1->>'content_hash'
    ) THEN
        RAISE EXCEPTION 'SDLC exact revision confirmation absent' USING ERRCODE = '23514';
    END IF;
    IF binding.state->>'stage'='Analysis' THEN
        IF target_name != 'SDLC Analysis' OR NEW.sprint_id IS NOT NULL OR NOT EXISTS (
            SELECT 1 FROM sdlc_analysis_intents q WHERE q.task_id=NEW.id
                AND q.requirement_revision=(binding.state->'revisions'->-1->>'revision')::bigint
                AND q.content_hash=binding.state->'revisions'->-1->>'content_hash'
        ) THEN
            RAISE EXCEPTION 'SDLC queued Analysis requires guarded admission' USING ERRCODE='23514';
        END IF;
    ELSIF binding.state->>'stage'='Architecture' THEN
        IF target_name != 'SDLC Architecture' OR NEW.sprint_id IS NOT NULL OR NOT EXISTS (
            SELECT 1 FROM sdlc_analysis_results r WHERE r.task_id=NEW.id
                AND r.payload->>'requirement_revision'=binding.state->'revisions'->-1->>'revision'
                AND r.payload->>'content_hash'=binding.state->'revisions'->-1->>'content_hash'
        ) THEN
            RAISE EXCEPTION 'SDLC Architecture requires accepted Analysis result' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER sdlc_history_immutable BEFORE UPDATE OR DELETE ON sdlc_analysis_results
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
CREATE TRIGGER sdlc_history_immutable BEFORE UPDATE OR DELETE ON sdlc_analysis_result_operations
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
            "Analysis result history is additive; rollback requires preserving lifecycle history"
                .into(),
        ))
    }
}
