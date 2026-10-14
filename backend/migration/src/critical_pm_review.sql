-- Included by MAIN after critical_pm_return.sql in the same UNRELEASED096 migration.
-- Reservation is pre-admission: no execution lease, native receipt or owner consent is created.
CREATE TABLE sdlc_critical_pm_reviews (
    review_id uuid PRIMARY KEY CHECK(review_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    blocker_id uuid NOT NULL UNIQUE,
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128
        AND idempotency_key NOT IN ('.','..') AND idempotency_key !~ '[[:space:][:cntrl:]]'
        AND position('/' in idempotency_key)=0 AND position(chr(92) in idempotency_key)=0),
    request_sha256 text NOT NULL CHECK(request_sha256 ~ '^[0-9a-f]{64}$'),
    command jsonb NOT NULL,
    previous_execution_id uuid NOT NULL,
    previous_owner_version bigint NOT NULL CHECK(previous_owner_version BETWEEN 1 AND 9007199254740990),
    execution_id uuid NOT NULL UNIQUE CHECK(execution_id<>'00000000-0000-0000-0000-000000000000'
        AND execution_id<>previous_execution_id),
    assignment_id uuid NOT NULL UNIQUE CHECK(assignment_id<>'00000000-0000-0000-0000-000000000000'
        AND assignment_id<>execution_id),
    assignment_version bigint NOT NULL CHECK(assignment_version BETWEEN 2 AND 9007199254740991),
    owner_version bigint NOT NULL CHECK(owner_version=previous_owner_version+1),
    transaction_id xid8 NOT NULL DEFAULT pg_current_xact_id(),
    before_state jsonb NOT NULL,
    after_state jsonb NOT NULL,
    before_execution jsonb NOT NULL,
    after_execution jsonb NOT NULL,
    after_blocker jsonb NOT NULL,
    result jsonb NOT NULL,
    UNIQUE(task_id,actor_subject,idempotency_key),
    FOREIGN KEY(blocker_id,task_id) REFERENCES sdlc_critical_blockers(blocker_id,task_id) ON DELETE RESTRICT,
    FOREIGN KEY(task_id,previous_execution_id) REFERENCES sdlc_pm_executions(task_id,execution_id) ON DELETE RESTRICT,
    FOREIGN KEY(task_id,execution_id) REFERENCES sdlc_pm_executions(task_id,execution_id)
        ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY(task_id,assignment_version) REFERENCES sdlc_assignments(task_id,version)
        ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY(review_id) REFERENCES sdlc_outbox(event_id) DEFERRABLE INITIALLY DEFERRED,
    CHECK ((command-ARRAY['blocker_id','blocker_sha256','expected_blocker_version','expected_task_version',
            'expected_pm_owner_version','expected_pm_assignment_version','idempotency_key']='{}'::jsonb
        AND result-ARRAY['purpose','reservation','context']='{}'::jsonb
        AND (result->'reservation')-ARRAY['contract_version','variant','binding','owner_cas','assignment',
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
        AND after_execution=jsonb_build_object('assignment',NULL,'lease',NULL,'binding',NULL,'result',NULL)) IS TRUE)
);
CREATE INDEX sdlc_critical_pm_review_task ON sdlc_critical_pm_reviews(task_id);
CREATE TRIGGER sdlc_critical_pm_review_immutable BEFORE UPDATE OR DELETE ON sdlc_critical_pm_reviews
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE FUNCTION sdlc_critical_pm_review_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks%ROWTYPE; previous jsonb; returned jsonb; original jsonb;
    expected_state jsonb; expected_blocker jsonb; next_pm jsonb;
BEGIN
    SELECT * INTO t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT result INTO previous FROM sdlc_pm_executions
        WHERE task_id=NEW.task_id AND execution_id=NEW.previous_execution_id;
    SELECT r.after_blocker INTO returned FROM sdlc_critical_pm_returns r
        JOIN sdlc_critical_blocker_heads h ON h.task_id=r.task_id AND h.blocker_id=r.blocker_id
        WHERE r.task_id=NEW.task_id AND r.blocker_id=NEW.blocker_id;
    SELECT payload INTO original FROM sdlc_assignments WHERE task_id=NEW.task_id ORDER BY version LIMIT 1;
    next_pm := NEW.result->'reservation';
    IF (t.root_task_id=t.task_id AND t.state->>'flow'='single-task-v1'
        AND NEW.transaction_id=pg_current_xact_id() AND NEW.before_state=t.state
        AND t.pm_execution_id=NEW.previous_execution_id AND t.pm_owner_version=NEW.previous_owner_version
        AND sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)
        AND t.state->'assignment'=previous->'assignment'
        AND t.state->>'stage'='Draft' AND t.state->>'status'='Ready'
        AND t.state->>'runtime_state'='terminal'
        AND t.state->>'waiting_reason'='critical_pm_review_queued'
        AND t.state->'lifecycle'->>'stage'='Draft' AND t.state->'lifecycle'->>'status'='Ready'
        AND t.state->'lifecycle'->>'runtime'='terminal'
        AND t.state->'lifecycle'->>'waiting_reason'='critical_pm_review_queued'
        AND (t.state->'lifecycle'->>'version')::bigint BETWEEN 1 AND 9007199254740990
        AND returned->>'state'='awaiting_pm_review'
        AND (returned->>'version')::bigint BETWEEN 1 AND 9007199254740990
        AND (returned->>'routing_generation')::bigint BETWEEN 1 AND 9007199254740991
        AND returned->'pm_review'='null'::jsonb AND returned->'resolution'='null'::jsonb
        AND returned->'technical_terminal'<>'null'::jsonb
        AND returned->>'task_id'=t.task_id::text AND returned->>'owner_subject'=t.owner_subject
        AND returned->>'project_id'=t.project_id::text AND returned->>'tracker_instance_id'=t.tracker_instance_id
        AND returned->'proposal'->'requirement_revision'=t.state->'lifecycle'->'routing'->'requirement_revision'
        AND returned->'proposal'->'content_hash'=t.state->'lifecycle'->'routing'->'content_hash'
        AND NEW.command->'expected_blocker_version'=returned->'version'
        AND NEW.command->'blocker_sha256'=returned->'blocker_sha256'
        AND NEW.command->'expected_task_version'=t.state->'lifecycle'->'version'
        AND NEW.command->'expected_pm_assignment_version'=previous->'assignment'->'version'
        AND NEW.assignment_version=(previous->'assignment'->>'version')::bigint+1
        AND next_pm->'binding'=previous->'binding'
        AND next_pm->'input'=previous->'input'
        AND next_pm->'assignment'->'agent_id'=original->'agent_id'
        AND next_pm->'assignment'->'agent_id'=previous->'assignment'->'agent_id'
        AND next_pm->'assignment'->'machine_subject'=previous->'assignment'->'machine_subject'
        AND NEW.assignment_id::text<>previous->'assignment'->>'assignment_id'
        AND next_pm->>'assignment_operation_key'='pm-draft:'||NEW.assignment_id::text
        AND next_pm->'execution'->>'ordinal' ~ '^[1-9][0-9]{0,18}$'
        AND (next_pm->'execution'->>'ordinal')::numeric BETWEEN 1 AND 9223372036854775807
        AND (next_pm->'execution'->>'ordinal')::numeric>(previous->'execution'->>'ordinal')::numeric
        AND next_pm->'execution'->>'key'='SDLC-'||(next_pm->'execution'->>'ordinal')) IS NOT TRUE
        OR EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=NEW.task_id)
        OR NOT EXISTS(SELECT 1 FROM sdlc_delivery_heads
            WHERE task_id=NEW.task_id AND execution=NEW.before_execution) THEN
        RAISE EXCEPTION 'SDLC critical PM review requires exact returned task and completed PM CAS' USING ERRCODE='23514';
    END IF;
    expected_state := jsonb_set(NEW.before_state,'{assignment}',next_pm->'assignment');
    expected_state := jsonb_set(expected_state,'{draft_submode}','"brainstorming"'::jsonb);
    expected_state := jsonb_set(expected_state,'{confirmation_revision}','null'::jsonb);
    expected_state := jsonb_set(expected_state,'{lifecycle,version}',
        to_jsonb((NEW.before_state->'lifecycle'->>'version')::bigint+1));
    expected_blocker := jsonb_set(returned,'{version}',to_jsonb((returned->>'version')::bigint+1));
    expected_blocker := jsonb_set(expected_blocker,'{state}','"in_pm_review"'::jsonb);
    expected_blocker := jsonb_set(expected_blocker,'{pm_review}',
        jsonb_build_object('assignment_id',NEW.assignment_id,'execution_id',NEW.execution_id));
    IF NEW.after_state IS DISTINCT FROM expected_state OR NEW.after_blocker IS DISTINCT FROM expected_blocker
        OR NEW.result->'context' IS DISTINCT FROM jsonb_build_object('blocker_id',NEW.blocker_id,
            'blocker_sha256',returned->'blocker_sha256','blocker_version',expected_blocker->'version',
            'requirements',jsonb_build_object('revision',returned->'proposal'->'requirement_revision',
                'content_hash',returned->'proposal'->'content_hash')) THEN
        RAISE EXCEPTION 'SDLC critical PM review changes reservation context only' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_critical_pm_review_insert_gate BEFORE INSERT ON sdlc_critical_pm_reviews
FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_review_insert_gate();

CREATE FUNCTION sdlc_critical_pm_review_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t JOIN sdlc_delivery_heads d USING(task_id)
        JOIN sdlc_pm_executions e ON e.task_id=t.task_id AND e.execution_id=NEW.execution_id
        JOIN sdlc_assignments a ON a.task_id=t.task_id AND a.version=NEW.assignment_version
        JOIN sdlc_outbox o ON o.task_id=t.task_id AND o.event_id=NEW.review_id
        WHERE t.task_id=NEW.task_id AND t.state=NEW.after_state AND d.execution=NEW.after_execution
            AND t.pm_execution_id=NEW.execution_id AND t.pm_owner_version=NEW.owner_version
            AND t.pm_admission_state='reserved' AND e.result=NEW.result->'reservation'
            AND a.payload=NEW.result->'reservation'->'assignment'
            AND o.event_type='critical.pm_review_reserved' AND o.payload->'result'=NEW.result)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_execution_leases WHERE execution_id=NEW.execution_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_native_admissions WHERE execution_id=NEW.execution_id)
        OR EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE execution_id=NEW.execution_id)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits
            WHERE task_id=NEW.task_id AND transaction_id=pg_current_xact_id())
        OR EXISTS(SELECT 1 FROM sdlc_pm_state_write_permits
            WHERE task_id=NEW.task_id AND transaction_id=pg_current_xact_id()) THEN
        RAISE EXCEPTION 'SDLC critical PM review must commit atomically without dispatch or lease' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_critical_pm_review_commit_gate AFTER INSERT ON sdlc_critical_pm_reviews
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_review_commit_gate();

CREATE FUNCTION sdlc_critical_pm_review_execution(p_task uuid,p_execution uuid,p_owner_version bigint,p_state jsonb)
RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_critical_pm_reviews j WHERE j.task_id=p_task
        AND j.execution_id=p_execution AND j.owner_version=p_owner_version
        AND j.result->'reservation'->'assignment'=p_state->'assignment')
$$;
ALTER TABLE sdlc_tasks DROP CONSTRAINT sdlc_pm_control;
ALTER TABLE sdlc_tasks ADD CONSTRAINT sdlc_pm_control CHECK (
    (pm_owner_version=0 AND pm_execution_id IS NULL AND pm_admission_state IS NULL)
    OR (pm_owner_version=1 AND pm_execution_id IS NOT NULL AND pm_admission_state IN ('reserved','completed')
        AND (state->'assignment'->>'execution_id'=pm_execution_id::text) IS TRUE)
    OR (pm_owner_version BETWEEN 2 AND 9007199254740991 AND pm_execution_id IS NOT NULL
        AND pm_admission_state IN ('reserved','completed')
        AND (state->'assignment'->>'execution_id'=pm_execution_id::text) IS TRUE
        AND sdlc_critical_pm_review_execution(task_id,pm_execution_id,pm_owner_version,state))
);

-- Transaction-bound exact transition predicate, not a dispatch or future-write permit.
CREATE FUNCTION sdlc_critical_pm_review_transition(p_task uuid,p_before_state jsonb,p_after_state jsonb,
    p_before_owner bigint,p_after_owner bigint,p_before_execution uuid,p_after_execution uuid,
    p_before_admission text,p_after_admission text) RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT p_before_admission='completed' AND p_after_admission='reserved'
        AND EXISTS(SELECT 1 FROM sdlc_critical_pm_reviews j WHERE j.task_id=p_task
            AND j.transaction_id=pg_current_xact_id() AND j.before_state=p_before_state AND j.after_state=p_after_state
            AND j.previous_owner_version=p_before_owner AND j.owner_version=p_after_owner
            AND j.previous_execution_id=p_before_execution AND j.execution_id=p_after_execution)
$$;

CREATE OR REPLACE FUNCTION sdlc_pm_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF sdlc_critical_pm_review_transition(OLD.task_id,OLD.state,NEW.state,OLD.pm_owner_version,NEW.pm_owner_version,
      OLD.pm_execution_id,NEW.pm_execution_id,OLD.pm_admission_state,NEW.pm_admission_state) THEN
    IF NEW.task_id IS DISTINCT FROM OLD.task_id OR NEW.root_task_id IS DISTINCT FROM OLD.root_task_id
        OR NEW.owner_subject IS DISTINCT FROM OLD.owner_subject OR NEW.project_id IS DISTINCT FROM OLD.project_id
        OR NEW.tracker_instance_id IS DISTINCT FROM OLD.tracker_instance_id
        OR NOT EXISTS(SELECT 1 FROM sdlc_pm_state_write_permits p WHERE p.task_id=OLD.task_id
            AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN
        RAISE EXCEPTION 'SDLC critical PM review requires exact canonical permit' USING ERRCODE='23514';
    END IF;
    DELETE FROM sdlc_pm_state_write_permits WHERE task_id=OLD.task_id
        AND transaction_id=pg_current_xact_id() AND next_state=NEW.state;
    RETURN NEW;
  END IF;
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
        AND sdlc_current_pm_completed(OLD.task_id,OLD.pm_execution_id,OLD.state,'completed')) THEN
        RAISE EXCEPTION 'SDLC reserved PM completion transition is not authorized' USING ERRCODE='23514';
      END IF;
    END IF;
    IF NEW.state IS DISTINCT FROM OLD.state THEN
      IF NOT (EXISTS(SELECT 1 FROM sdlc_pm_native_admissions a WHERE a.execution_id=OLD.pm_execution_id
          AND a.assignment_id::text=(OLD.state #>> '{assignment,assignment_id}')
          AND a.assignment_version::text=(OLD.state #>> '{assignment,version}')
          AND (a.proof #>> '{identity,task_ref}')=OLD.task_id::text)
        OR sdlc_current_pm_completed(OLD.task_id,OLD.pm_execution_id,OLD.state,OLD.pm_admission_state)
        OR sdlc_current_pm_draft_reserved(OLD.task_id,OLD.pm_execution_id,OLD.state,
            OLD.pm_owner_version,OLD.pm_admission_state)) THEN
        RAISE EXCEPTION 'SDLC reserved PM requires verified reservation, completion or native admission' USING ERRCODE='23514';
      END IF;
      IF NOT EXISTS(SELECT 1 FROM sdlc_pm_state_write_permits p WHERE p.task_id=OLD.task_id
          AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN
        RAISE EXCEPTION 'SDLC reserved PM requires an authorized state write' USING ERRCODE='23514';
      END IF;
      DELETE FROM sdlc_pm_state_write_permits WHERE task_id=OLD.task_id
          AND transaction_id=pg_current_xact_id() AND next_state=NEW.state;
    END IF;
  END IF;
  RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION sdlc_delivery_legacy_exclusion() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE selected_agent text; selected_task uuid;
BEGIN
    selected_task := NEW.task_id; selected_agent := NEW.payload->>'agent_id';
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF TG_TABLE_NAME='sdlc_assignments' THEN
        IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds
            WHERE task_id=selected_task OR agent_id::text=selected_agent)
            AND EXISTS(SELECT 1 FROM sdlc_critical_pm_reviews j JOIN sdlc_tasks t ON t.task_id=j.task_id
                WHERE j.task_id=selected_task AND j.transaction_id=pg_current_xact_id()
                    AND j.before_state=t.state AND j.assignment_version=NEW.version
                    AND j.result->'reservation'->'assignment'=NEW.payload) THEN RETURN NEW; END IF;
    END IF;
    IF EXISTS(SELECT 1 FROM sdlc_delivery_tasks WHERE task_id=selected_task)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE agent_id::text=selected_agent) THEN
        RAISE EXCEPTION 'SDLC single-task delivery owns execution' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION sdlc_critical_projection_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE active_before jsonb; expected jsonb; returned_before jsonb; returned_after jsonb;
BEGIN
    IF TG_TABLE_NAME='sdlc_tasks' THEN
        IF sdlc_critical_pm_review_transition(OLD.task_id,OLD.state,NEW.state,
            OLD.pm_owner_version,NEW.pm_owner_version,OLD.pm_execution_id,NEW.pm_execution_id,
            OLD.pm_admission_state,NEW.pm_admission_state) THEN RETURN NEW; END IF;
    END IF;
    SELECT b.previous_receipt INTO active_before FROM sdlc_critical_blocker_heads h JOIN sdlc_critical_blockers b
        ON b.blocker_id=h.blocker_id AND b.task_id=h.task_id WHERE h.task_id=NEW.task_id;
    IF NOT FOUND THEN
        IF TG_TABLE_NAME='sdlc_tasks' THEN
            IF NEW.state->'lifecycle'->>'waiting_reason'='critical_business_question_held' THEN
                RAISE EXCEPTION 'SDLC critical waiting requires a persisted blocker' USING ERRCODE='23514';
            END IF;
        END IF;
        RETURN NEW;
    END IF;
    IF TG_TABLE_NAME='sdlc_tasks' THEN
        SELECT r.before_receipt,r.after_receipt INTO returned_before,returned_after
            FROM sdlc_critical_pm_returns r JOIN sdlc_critical_blocker_heads h
                ON h.task_id=r.task_id AND h.blocker_id=r.blocker_id WHERE r.task_id=NEW.task_id;
        IF FOUND AND OLD.state->'lifecycle'=returned_before->'task'
            AND NEW.state->'lifecycle'=returned_after->'task' THEN
            expected := jsonb_set(OLD.state,'{lifecycle}',returned_after->'task');
            expected := jsonb_set(expected,'{stage}',returned_after->'task'->'stage');
            expected := jsonb_set(expected,'{status}',returned_after->'task'->'status');
            expected := jsonb_set(expected,'{runtime_state}',returned_after->'task'->'runtime');
            expected := jsonb_set(expected,'{waiting_reason}',returned_after->'task'->'waiting_reason');
            IF NEW.state IS DISTINCT FROM expected THEN
                RAISE EXCEPTION 'SDLC critical return cannot change unrelated state' USING ERRCODE='23514';
            END IF;
            RETURN NEW;
        END IF;
        IF OLD.state->'lifecycle'->>'waiting_reason'='critical_business_question_held' THEN
            IF NEW.state IS DISTINCT FROM OLD.state THEN
                RAISE EXCEPTION 'SDLC critical hold forbids business progression' USING ERRCODE='23514';
            END IF;
        ELSE
            expected := jsonb_set(OLD.state,'{waiting_reason}','"critical_business_question_held"'::jsonb);
            expected := jsonb_set(expected,'{lifecycle,waiting_reason}','"critical_business_question_held"'::jsonb);
            expected := jsonb_set(expected,'{lifecycle,version}',to_jsonb((OLD.state->'lifecycle'->>'version')::bigint+1));
            IF NEW.state IS DISTINCT FROM expected
                OR OLD.state->'lifecycle' IS DISTINCT FROM active_before->'task' THEN
                RAISE EXCEPTION 'SDLC critical hold changes waiting only' USING ERRCODE='23514';
            END IF;
        END IF;
    ELSE
        IF EXISTS(SELECT 1 FROM sdlc_critical_pm_returns r
            WHERE r.task_id=NEW.task_id AND r.before_receipt-'task'=OLD.execution
                AND r.after_receipt-'task'=NEW.execution) THEN RETURN NEW; END IF;
        IF NEW.execution-'lease' IS DISTINCT FROM OLD.execution-'lease' THEN
            RAISE EXCEPTION 'SDLC critical hold retains exact assignment/run custody' USING ERRCODE='23514';
        END IF;
        -- Existing sdlc_delivery_head_gate verifies every actual lease CAS/TTL update.
    END IF;
    RETURN NEW;
END $$;
