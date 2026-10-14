-- COMPANION ONLY: main includes AFTER critical_pm_review.sql in its ONE unreleased096.
-- No native proof, scheduler, public command or independently committed completion is added.
CREATE TABLE sdlc_critical_pm_lifecycle_operations (
    operation_id uuid PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    blocker_id uuid NOT NULL REFERENCES sdlc_critical_pm_reviews(blocker_id) ON DELETE RESTRICT,
    execution_id uuid NOT NULL REFERENCES sdlc_pm_executions(execution_id) ON DELETE RESTRICT,
    owner_version bigint NOT NULL CHECK(owner_version BETWEEN 2 AND 9007199254740991),
    actor_subject text NOT NULL CHECK(length(actor_subject)>0),
    kind text NOT NULL CHECK(kind IN ('claim','command','resume','complete')),
    -- Existing commands keep their original key policy, including opaque punctuation.
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128
        AND idempotency_key !~ '[[:space:][:cntrl:]]'),
    request_sha256 text NOT NULL CHECK(request_sha256 ~ '^[0-9a-f]{64}$'),
    command jsonb NOT NULL, result jsonb NOT NULL,
    lease_id uuid NOT NULL CHECK(lease_id<>'00000000-0000-0000-0000-000000000000'),
    lease_version bigint NOT NULL CHECK(lease_version BETWEEN 1 AND 9007199254740991),
    before_state jsonb NOT NULL, after_state jsonb NOT NULL,
    before_execution jsonb NOT NULL, after_execution jsonb NOT NULL,
    before_blocker jsonb NOT NULL, after_blocker jsonb NOT NULL,
    transaction_id xid8 NOT NULL DEFAULT pg_current_xact_id(),
    outbox_event_id uuid NOT NULL UNIQUE REFERENCES sdlc_outbox(event_id) DEFERRABLE INITIALLY DEFERRED,
    completion_id uuid UNIQUE REFERENCES sdlc_pm_completions(completion_id) DEFERRABLE INITIALLY DEFERRED,
    CHECK((kind='complete')=(completion_id IS NOT NULL)),
    CHECK(before_execution=after_execution AND before_execution=
        jsonb_build_object('assignment',NULL,'lease',NULL,'binding',NULL,'result',NULL))
);
CREATE UNIQUE INDEX sdlc_critical_pm_command_original ON sdlc_critical_pm_lifecycle_operations
    (task_id,actor_subject,idempotency_key) WHERE kind='command';
CREATE UNIQUE INDEX sdlc_critical_pm_claim_original ON sdlc_critical_pm_lifecycle_operations
    (execution_id,actor_subject,idempotency_key) WHERE kind='claim';
CREATE UNIQUE INDEX sdlc_critical_pm_resume_original ON sdlc_critical_pm_lifecycle_operations
    (task_id,actor_subject,idempotency_key) WHERE kind='resume';
CREATE UNIQUE INDEX sdlc_critical_pm_complete_original ON sdlc_critical_pm_lifecycle_operations
    (task_id,idempotency_key) WHERE kind='complete';
CREATE UNIQUE INDEX sdlc_critical_pm_complete_once ON sdlc_critical_pm_lifecycle_operations
    (blocker_id) WHERE kind='complete';
CREATE TRIGGER sdlc_critical_pm_lifecycle_immutable BEFORE UPDATE OR DELETE ON sdlc_critical_pm_lifecycle_operations
    FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE TABLE sdlc_task_routing_generations (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    generation bigint NOT NULL CHECK(generation BETWEEN 2 AND 9007199254740991),
    snapshot_id uuid NOT NULL UNIQUE CHECK(snapshot_id<>'00000000-0000-0000-0000-000000000000'),
    previous_snapshot_id uuid NOT NULL CHECK(previous_snapshot_id<>snapshot_id),
    blocker_id uuid NOT NULL UNIQUE REFERENCES sdlc_critical_pm_reviews(blocker_id) ON DELETE RESTRICT,
    execution_id uuid NOT NULL REFERENCES sdlc_pm_executions(execution_id) ON DELETE RESTRICT,
    completion_id uuid NOT NULL UNIQUE REFERENCES sdlc_pm_completions(completion_id) DEFERRABLE INITIALLY DEFERRED,
    operation_id uuid NOT NULL UNIQUE REFERENCES sdlc_critical_pm_lifecycle_operations(operation_id),
    confirmation_id uuid NOT NULL UNIQUE, requirement_revision bigint NOT NULL, content_hash text NOT NULL,
    project_id uuid NOT NULL, policy_version bigint NOT NULL, payload jsonb NOT NULL,
    PRIMARY KEY(task_id,generation),
    UNIQUE(task_id,generation,snapshot_id),
    UNIQUE(task_id,confirmation_id,requirement_revision,content_hash,snapshot_id),
    FOREIGN KEY(task_id,project_id) REFERENCES sdlc_tasks(task_id,project_id),
    FOREIGN KEY(project_id,policy_version) REFERENCES sdlc_project_routing_revisions(project_id,version),
    FOREIGN KEY(task_id,confirmation_id,requirement_revision,content_hash)
        REFERENCES sdlc_confirmations(task_id,id,revision,content_hash),
    CHECK((payload->'contract_version'='2'::jsonb AND payload->>'snapshot_id'=snapshot_id::text
        AND payload->>'task_id'=task_id::text AND payload->>'root_task_id'=task_id::text
        AND payload->>'project_id'=project_id::text AND payload->>'confirmation_id'=confirmation_id::text
        AND payload->'requirement_revision'=to_jsonb(requirement_revision) AND payload->>'content_hash'=content_hash
        AND payload->'policy'->'version'=to_jsonb(policy_version)) IS TRUE)
);
CREATE TRIGGER sdlc_task_routing_generation_immutable BEFORE UPDATE OR DELETE ON sdlc_task_routing_generations
    FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

-- Generation1 is a read-through union, never copied/backfilled/updated.
CREATE VIEW sdlc_routing_snapshot_inventory AS
    SELECT snapshot_id,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,payload,
        1::bigint AS generation FROM sdlc_task_routing_snapshots
    UNION ALL SELECT snapshot_id,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,payload,
        generation FROM sdlc_task_routing_generations;

CREATE FUNCTION sdlc_critical_pm_detail(p_blocker uuid) RETURNS jsonb LANGUAGE sql STABLE AS $$
    SELECT COALESCE(o.after_blocker,v.after_blocker,r.after_blocker,b.payload)
    FROM sdlc_critical_blockers b
    LEFT JOIN sdlc_critical_pm_returns r ON r.blocker_id=b.blocker_id AND r.task_id=b.task_id
    LEFT JOIN sdlc_critical_pm_reviews v ON v.blocker_id=b.blocker_id AND v.task_id=b.task_id
    LEFT JOIN sdlc_critical_pm_lifecycle_operations o ON o.blocker_id=b.blocker_id AND o.task_id=b.task_id AND o.kind='complete'
    WHERE b.blocker_id=p_blocker
$$;

CREATE FUNCTION sdlc_critical_pm_retained_head_allowed(p_task uuid,p_execution uuid)
RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_tasks t JOIN sdlc_critical_pm_reviews v ON v.task_id=t.task_id
        JOIN sdlc_critical_blocker_heads h ON h.task_id=v.task_id AND h.blocker_id=v.blocker_id
        JOIN sdlc_critical_pm_returns r ON r.task_id=v.task_id AND r.blocker_id=v.blocker_id
        JOIN sdlc_delivery_heads d ON d.task_id=t.task_id
        WHERE t.task_id=p_task AND t.pm_execution_id=p_execution AND v.execution_id=p_execution
            AND t.pm_admission_state='reserved' AND t.pm_owner_version=v.owner_version
            AND t.state->'assignment'=v.result->'reservation'->'assignment'
            AND t.state->>'stage'='Draft' AND t.state->>'flow'='single-task-v1'
            AND sdlc_critical_pm_detail(h.blocker_id)->>'state'='in_pm_review'
            AND r.after_blocker->'technical_terminal'<>'null'::jsonb
            AND d.execution=jsonb_build_object('assignment',NULL,'lease',NULL,'binding',NULL,'result',NULL)
            AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=p_task)
            AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=p_task))
$$;

-- FK families are exclusive. Legacy unrouted history stays valid only on its existing legacy branch.
ALTER TABLE sdlc_analysis_intents ADD COLUMN routing_generation bigint,
    ADD COLUMN review_routing_snapshot_id uuid REFERENCES sdlc_task_routing_generations(snapshot_id);
ALTER TABLE sdlc_analysis_intents ADD CONSTRAINT sdlc_analysis_intent_routing_family CHECK (
    ((routing_generation IS NULL AND review_routing_snapshot_id IS NULL)
    OR (routing_generation BETWEEN 2 AND 9007199254740991 AND review_routing_snapshot_id IS NOT NULL
        AND routing_snapshot_id IS NULL)) IS TRUE
);
ALTER TABLE sdlc_analysis_intents ADD CONSTRAINT sdlc_analysis_intent_generation_identity
    FOREIGN KEY(task_id,routing_generation,review_routing_snapshot_id)
    REFERENCES sdlc_task_routing_generations(task_id,generation,snapshot_id);
ALTER TABLE sdlc_analysis_intents ADD CONSTRAINT sdlc_analysis_intent_generation_confirmation
    FOREIGN KEY(task_id,confirmation_id,requirement_revision,content_hash,review_routing_snapshot_id)
    REFERENCES sdlc_task_routing_generations(task_id,confirmation_id,requirement_revision,content_hash,snapshot_id);

CREATE OR REPLACE FUNCTION sdlc_analysis_routing_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE expected uuid; workflow text;
BEGIN
    IF NEW.review_routing_snapshot_id IS NOT NULL THEN
        IF NEW.routing_snapshot_id IS NOT NULL OR NEW.routing_generation IS NULL THEN
            RAISE EXCEPTION 'SDLC exactly one routed intent FK required' USING ERRCODE='23514';
        END IF;
        SELECT snapshot_id,payload->'policy'->'routes'->'analyst'->>'workflow_key' INTO expected,workflow
            FROM sdlc_task_routing_generations WHERE task_id=NEW.task_id AND generation=NEW.routing_generation
                AND snapshot_id=NEW.review_routing_snapshot_id AND confirmation_id=NEW.confirmation_id
                AND requirement_revision=NEW.requirement_revision AND content_hash=NEW.content_hash;
        IF expected IS NULL THEN RAISE EXCEPTION 'SDLC exact intent generation missing' USING ERRCODE='23514'; END IF;
    ELSE
        SELECT snapshot_id,payload->'policy'->'routes'->'analyst'->>'workflow_key' INTO expected,workflow
            FROM sdlc_task_routing_snapshots WHERE task_id=NEW.task_id;
        IF NEW.routing_generation IS NOT NULL OR NEW.routing_snapshot_id IS DISTINCT FROM expected THEN
            RAISE EXCEPTION 'SDLC initial intent routing FK mismatch' USING ERRCODE='23514';
        END IF;
    END IF;
    IF NEW.payload->>'workflow' IS DISTINCT FROM COALESCE(workflow,'hermes-sdlc:analyst') THEN
        RAISE EXCEPTION 'SDLC exact generation Analyst workflow mismatch' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE FUNCTION sdlc_critical_pm_lifecycle_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks%ROWTYPE; v sdlc_critical_pm_reviews%ROWTYPE; l sdlc_pm_execution_leases%ROWTYPE;
    head jsonb; detail jsonb; expected jsonb; next_task jsonb; expected_blocker jsonb; body jsonb;
    op text; questions jsonb; q jsonb; prior jsonb; revision jsonb; confirmation jsonb;
BEGIN
    SELECT * INTO t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT * INTO v FROM sdlc_critical_pm_reviews WHERE task_id=t.task_id AND blocker_id=NEW.blocker_id;
    SELECT execution INTO head FROM sdlc_delivery_heads WHERE task_id=t.task_id;
    SELECT * INTO l FROM sdlc_pm_execution_leases WHERE execution_id=NEW.execution_id;
    detail:=sdlc_critical_pm_detail(NEW.blocker_id); revision:=t.state->'revisions'->-1;
    IF (NEW.transaction_id=pg_current_xact_id() AND t.task_id=t.root_task_id AND t.state->>'flow'='single-task-v1'
        AND t.state->>'stage'='Draft' AND t.pm_admission_state='reserved'
        AND t.pm_execution_id=NEW.execution_id AND v.execution_id=NEW.execution_id
        AND t.pm_owner_version=NEW.owner_version AND v.owner_version=NEW.owner_version
        AND t.state->'assignment'=v.result->'reservation'->'assignment'
        AND NEW.before_state=t.state AND NEW.before_execution=head AND NEW.before_blocker=detail
        AND detail->>'state'='in_pm_review' AND detail->'resolution'='null'::jsonb
        AND (t.state->'lifecycle'->>'version')::bigint BETWEEN 1 AND 9007199254740990
        AND l.lease_id=NEW.lease_id AND l.version=NEW.lease_version
        AND l.result->'fence'=jsonb_build_object('assignment_id',t.state->'assignment'->'assignment_id',
            'execution_id',t.state->'assignment'->'execution_id','agent_id',t.state->'assignment'->'agent_id',
            'assignment_version',t.state->'assignment'->'version')
        AND sdlc_critical_pm_retained_head_allowed(t.task_id,NEW.execution_id)) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC exact current review journal source required' USING ERRCODE='23514';
    END IF;
    expected:=NEW.before_state; next_task:=expected->'lifecycle'; expected_blocker:=detail;
    next_task:=jsonb_set(next_task,'{version}',to_jsonb((next_task->>'version')::bigint+1));
    IF NEW.kind IN ('claim','resume') THEN
        IF NEW.command->>'idempotency_key' IS DISTINCT FROM NEW.idempotency_key
            OR NEW.result IS DISTINCT FROM l.result OR NEW.result->'owner_version' IS DISTINCT FROM to_jsonb(NEW.owner_version)
            OR (NEW.kind='claim' AND (NEW.actor_subject IS DISTINCT FROM l.holder_subject OR l.version<>1)) THEN
            RAISE EXCEPTION 'SDLC review lease journal differs from actual lease' USING ERRCODE='23514';
        END IF;
        next_task:=jsonb_set(next_task,'{runtime}','"reserved"'::jsonb);
        IF next_task->>'waiting_reason' IS DISTINCT FROM 'critical_pm_awaiting_finalization' THEN
            next_task:=jsonb_set(next_task,'{status}','"InProgress"'::jsonb);
        END IF;
    ELSIF NEW.kind='command' THEN
        op:=NEW.command->>'operation'; body:=NEW.command->'payload'; questions:=expected->'questions';
        IF COALESCE(body->'command'->>'idempotency_key',body->>'idempotency_key') IS DISTINCT FROM NEW.idempotency_key THEN
            RAISE EXCEPTION 'SDLC review command original key differs' USING ERRCODE='23514';
        END IF;
        IF op IN ('publish_question','publish_revision','cancel') AND
            (NEW.actor_subject IS DISTINCT FROM l.holder_subject OR l.expires_at<=clock_timestamp()) THEN
            RAISE EXCEPTION 'SDLC review producer needs actual live current lease' USING ERRCODE='23514';
        END IF;
        IF op='publish_revision' THEN
            IF body->'fence' IS DISTINCT FROM l.result->'fence'
                OR body->'expected_requirement_revision' IS DISTINCT FROM revision->'revision'
                OR NEW.result->'revision' IS DISTINCT FROM to_jsonb((revision->>'revision')::bigint+1)
                OR (NEW.result-ARRAY['revision','content_hash','author_subject','created_at']) IS DISTINCT FROM body->'document'
                OR NEW.result->>'author_subject' IS DISTINCT FROM NEW.actor_subject
                OR EXISTS(SELECT 1 FROM jsonb_array_elements(questions) x WHERE x->>'required'='true' AND x->>'state'='open') THEN
                RAISE EXCEPTION 'SDLC review revision differs from actual producer command' USING ERRCODE='23514';
            END IF;
            expected:=jsonb_set(expected,'{revisions}',(expected->'revisions')||jsonb_build_array(NEW.result));
            SELECT COALESCE(jsonb_agg(CASE WHEN x->>'state'='open' THEN jsonb_set(x,'{state}','"superseded"'::jsonb) ELSE x END),'[]'::jsonb)
                INTO questions FROM jsonb_array_elements(questions) x;
            expected:=jsonb_set(expected,'{questions}',questions);
            expected:=jsonb_set(expected,'{confirmation_revision}',NEW.result->'revision');
            expected:=jsonb_set(expected,'{draft_submode}','"awaiting_confirmation"'::jsonb);
            next_task:=jsonb_set(next_task,'{status}','"Ready"'::jsonb);
            next_task:=jsonb_set(next_task,'{waiting_reason}','"critical_pm_review_queued"'::jsonb);
        ELSIF op='publish_question' THEN
            IF body->'fence' IS DISTINCT FROM l.result->'fence' OR body->'requirement_revision' IS DISTINCT FROM revision->'revision'
                OR NEW.result->'assignment_id' IS DISTINCT FROM body->'fence'->'assignment_id'
                OR NEW.result->'execution_id' IS DISTINCT FROM body->'fence'->'execution_id'
                OR NEW.result->'agent_id' IS DISTINCT FROM body->'fence'->'agent_id'
                OR NEW.result->'assignment_version' IS DISTINCT FROM body->'fence'->'assignment_version'
                OR NEW.result->'id' IS DISTINCT FROM body->'question_id' OR NEW.result->'request_id' IS DISTINCT FROM body->'request_id'
                OR NEW.result->'checkpoint_id' IS DISTINCT FROM body->'checkpoint_id'
                OR NEW.result->'requirement_revision' IS DISTINCT FROM body->'requirement_revision'
                OR NEW.result->>'task_id' IS DISTINCT FROM t.task_id::text
                OR NEW.result->>'root_task_id' IS DISTINCT FROM t.task_id::text
                OR (NEW.result-ARRAY['id','task_id','root_task_id','version','assignment_id','execution_id','agent_id',
                    'assignment_version','state','answer','author_subject','created_at']) IS DISTINCT FROM
                    (body-ARRAY['fence','question_id','expected_question_version','idempotency_key'])
                OR NEW.result->>'author_subject' IS DISTINCT FROM NEW.actor_subject
                OR NEW.result->>'state' IS DISTINCT FROM 'open' OR NEW.result->'answer' IS DISTINCT FROM 'null'::jsonb THEN
                RAISE EXCEPTION 'SDLC review question differs from actual producer fence' USING ERRCODE='23514';
            END IF;
            SELECT x INTO prior FROM jsonb_array_elements(questions) x WHERE x->'id'=NEW.result->'id';
            IF prior IS NULL THEN
                IF NEW.result->'version' IS DISTINCT FROM '1'::jsonb
                    OR body->'expected_question_version' IS DISTINCT FROM 'null'::jsonb THEN
                    RAISE EXCEPTION 'SDLC new review question needs initial version' USING ERRCODE='23514';
                END IF;
                questions:=questions||jsonb_build_array(NEW.result);
            ELSE
                IF prior->'assignment_id' IS DISTINCT FROM l.result->'fence'->'assignment_id'
                    OR prior->'execution_id' IS DISTINCT FROM l.result->'fence'->'execution_id'
                    OR prior->>'state' IS DISTINCT FROM 'open'
                    OR body->'expected_question_version' IS DISTINCT FROM prior->'version'
                    OR NEW.result->'version' IS DISTINCT FROM to_jsonb((prior->>'version')::bigint+1) THEN
                    RAISE EXCEPTION 'SDLC previous PM question cannot be rewritten' USING ERRCODE='23514';
                END IF;
                SELECT jsonb_agg(CASE WHEN x->'id'=NEW.result->'id' THEN NEW.result ELSE x END)
                    INTO questions FROM jsonb_array_elements(questions) x;
            END IF;
            expected:=jsonb_set(expected,'{questions}',questions);
            expected:=jsonb_set(expected,'{confirmation_revision}','null'::jsonb);
            expected:=jsonb_set(expected,'{draft_submode}','"clarification"'::jsonb);
            next_task:=jsonb_set(next_task,'{status}','"Ready"'::jsonb);
            next_task:=jsonb_set(next_task,'{waiting_reason}','"critical_pm_review_queued"'::jsonb);
        ELSIF op IN ('answer','cancel') THEN
            SELECT x INTO prior FROM jsonb_array_elements(questions) x WHERE x->'id'=body->'question_id';
            IF prior IS NULL OR prior->>'state'<>'open'
                OR prior->'version' IS DISTINCT FROM body->'command'->'expected_question_version'
                OR prior->'assignment_id' IS DISTINCT FROM l.result->'fence'->'assignment_id'
                OR prior->'execution_id' IS DISTINCT FROM l.result->'fence'->'execution_id' THEN
                RAISE EXCEPTION 'SDLC exact current review question required' USING ERRCODE='23514';
            END IF;
            IF op='answer' THEN
                IF NEW.actor_subject IS DISTINCT FROM t.owner_subject OR NEW.result->>'author_subject' IS DISTINCT FROM t.owner_subject
                    OR NEW.result->'question_id' IS DISTINCT FROM prior->'id'
                    OR NEW.result->'question_version' IS DISTINCT FROM prior->'version'
                    OR NEW.result->'requirement_revision' IS DISTINCT FROM revision->'revision'
                    OR NEW.result->'selected_option_ids' IS DISTINCT FROM body->'command'->'selected_option_ids'
                    OR NEW.result->'text' IS DISTINCT FROM body->'command'->'text'
                    OR NEW.result->'comment' IS DISTINCT FROM body->'command'->'comment' THEN
                    RAISE EXCEPTION 'SDLC actual owner answer required' USING ERRCODE='23514';
                END IF;
                q:=jsonb_set(jsonb_set(prior,'{answer}',NEW.result),'{state}','"answered"'::jsonb);
            ELSE
                q:=jsonb_set(prior,'{state}','"cancelled"'::jsonb);
                IF NEW.result IS DISTINCT FROM q OR body->'command'->'fence' IS DISTINCT FROM l.result->'fence' THEN
                    RAISE EXCEPTION 'SDLC exact review cancellation required' USING ERRCODE='23514';
                END IF;
            END IF;
            SELECT jsonb_agg(CASE WHEN x->'id'=q->'id' THEN q ELSE x END) INTO questions FROM jsonb_array_elements(questions) x;
            expected:=jsonb_set(expected,'{questions}',questions);
            next_task:=jsonb_set(next_task,'{status}',CASE WHEN EXISTS(SELECT 1 FROM jsonb_array_elements(questions) x WHERE x->>'state'='open')
                THEN '"Ready"'::jsonb ELSE '"InProgress"'::jsonb END);
        ELSIF op='evidence' THEN
            IF body->'fence' IS DISTINCT FROM l.result->'fence'
                OR NEW.result-'created_at'-'verifier_subject' IS DISTINCT FROM body-'fence'-'idempotency_key'
                OR NEW.result->>'verifier_subject' IS DISTINCT FROM NEW.actor_subject
                OR NEW.result->'requirement_revision' IS DISTINCT FROM revision->'revision'
                OR NEW.result->'content_hash' IS DISTINCT FROM revision->'content_hash' THEN
                RAISE EXCEPTION 'SDLC actual review evidence required' USING ERRCODE='23514';
            END IF;
            expected:=jsonb_set(expected,'{evidence}',(expected->'evidence')||jsonb_build_array(NEW.result));
        ELSIF op='confirm' THEN
            IF NEW.actor_subject IS DISTINCT FROM t.owner_subject OR NEW.result->>'owner_subject' IS DISTINCT FROM t.owner_subject
                OR NEW.result->>'stage' IS DISTINCT FROM 'Backlog' OR NEW.result->>'task_id' IS DISTINCT FROM t.task_id::text
                OR NEW.result->'revision' IS DISTINCT FROM revision->'revision'
                OR NEW.result->'content_hash' IS DISTINCT FROM revision->'content_hash'
                OR body->'revision' IS DISTINCT FROM revision->'revision'
                OR body->'command'->'content_hash' IS DISTINCT FROM revision->'content_hash'
                OR (revision->>'revision')::bigint <= (v.result->'context'->'requirements'->>'revision')::bigint
                OR EXISTS(SELECT 1 FROM sdlc_confirmations WHERE task_id=t.task_id AND revision=(NEW.result->>'revision')::bigint) THEN
                RAISE EXCEPTION 'SDLC new exact owner confirmation required' USING ERRCODE='23514';
            END IF;
            expected:=jsonb_set(expected,'{confirmations}',(expected->'confirmations')||jsonb_build_array(NEW.result));
            expected:=jsonb_set(expected,'{draft_submode}','"awaiting_confirmation"'::jsonb);
            next_task:=jsonb_set(next_task,'{status}','"Ready"'::jsonb);
            next_task:=jsonb_set(next_task,'{waiting_reason}','"critical_pm_awaiting_finalization"'::jsonb);
        ELSE RAISE EXCEPTION 'SDLC unsupported review command' USING ERRCODE='23514'; END IF;
    ELSE
        IF EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE execution_id=NEW.execution_id) THEN
            RAISE EXCEPTION 'SDLC current review completion already exists' USING ERRCODE='23514';
        END IF;
        SELECT c.payload INTO confirmation FROM sdlc_confirmations c WHERE c.task_id=t.task_id
            AND c.revision=(revision->>'revision')::bigint AND c.content_hash=revision->>'content_hash';
        IF confirmation IS NULL OR confirmation->>'owner_subject' IS DISTINCT FROM t.owner_subject
            OR t.state->'confirmation_revision' IS DISTINCT FROM revision->'revision'
            OR t.state->'lifecycle'->>'waiting_reason' IS DISTINCT FROM 'critical_pm_awaiting_finalization'
            OR (revision->>'revision')::bigint <= (v.result->'context'->'requirements'->>'revision')::bigint
        OR NEW.command IS DISTINCT FROM NEW.result-ARRAY['completion_id','tracker_instance_id','project_id','task_id',
                'root_task_id','owner_subject','owner_version','previous_pm_admission_state','next_pm_admission_state','created_at']
            OR NEW.result->>'completion_id' IS DISTINCT FROM NEW.completion_id::text
            OR NEW.result->'fence' IS DISTINCT FROM l.result->'fence' OR NEW.result->>'lease_id' IS DISTINCT FROM l.lease_id::text
            OR NEW.result->'lease_version' IS DISTINCT FROM to_jsonb(l.version)
            OR NEW.result->'requirement_revision' IS DISTINCT FROM revision->'revision'
            OR NEW.result->'content_hash' IS DISTINCT FROM revision->'content_hash'
            OR NEW.command->>'idempotency_key' IS DISTINCT FROM NEW.idempotency_key THEN
            RAISE EXCEPTION 'SDLC exact same-transaction PM finalization required' USING ERRCODE='23514';
        END IF;
        expected_blocker:=jsonb_set(detail,'{version}',to_jsonb((detail->>'version')::bigint+1));
        expected_blocker:=jsonb_set(expected_blocker,'{state}','"resolved"'::jsonb);
        expected_blocker:=jsonb_set(expected_blocker,'{resolution}',jsonb_build_object('requirement_revision',revision->'revision',
            'content_hash',revision->'content_hash','routing_generation',(detail->>'routing_generation')::bigint+1));
        next_task:=jsonb_set(next_task,'{stage}','"Analysis"'::jsonb);
        next_task:=jsonb_set(next_task,'{status}','"Ready"'::jsonb);
        next_task:=jsonb_set(next_task,'{runtime}','"queued"'::jsonb);
        next_task:=jsonb_set(next_task,'{waiting_reason}','"queued_for_assignment"'::jsonb);
        next_task:=jsonb_set(next_task,'{cycle}','0'::jsonb);
        next_task:=jsonb_set(next_task,'{transient_retries}','0'::jsonb);
        next_task:=jsonb_set(next_task,'{routing}',NEW.after_state->'lifecycle'->'routing');
        expected:=expected-'draft_submode';
    END IF;
    expected:=jsonb_set(expected,'{lifecycle}',next_task);
    expected:=jsonb_set(expected,'{stage}',next_task->'stage'); expected:=jsonb_set(expected,'{status}',next_task->'status');
    expected:=jsonb_set(expected,'{runtime_state}',next_task->'runtime');
    expected:=jsonb_set(expected,'{waiting_reason}',next_task->'waiting_reason');
    IF NEW.after_state IS DISTINCT FROM expected OR NEW.after_blocker IS DISTINCT FROM expected_blocker THEN
        RAISE EXCEPTION 'SDLC review journal cannot change unrelated state/history' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_critical_pm_lifecycle_insert_gate BEFORE INSERT ON sdlc_critical_pm_lifecycle_operations
    FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_lifecycle_insert_gate();

CREATE FUNCTION sdlc_critical_pm_generation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE j sdlc_critical_pm_lifecycle_operations%ROWTYPE; previous jsonb; previous_generation bigint;
BEGIN
    SELECT * INTO j FROM sdlc_critical_pm_lifecycle_operations WHERE operation_id=NEW.operation_id;
    SELECT payload,generation INTO previous,previous_generation FROM sdlc_routing_snapshot_inventory
        WHERE task_id=NEW.task_id AND snapshot_id=NEW.previous_snapshot_id;
    IF (j.kind='complete' AND j.transaction_id=pg_current_xact_id() AND j.task_id=NEW.task_id
        AND j.blocker_id=NEW.blocker_id AND j.execution_id=NEW.execution_id AND j.completion_id=NEW.completion_id
        AND NEW.generation=previous_generation+1
        AND NOT EXISTS(SELECT 1 FROM sdlc_task_routing_snapshots s WHERE s.snapshot_id=NEW.snapshot_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_task_routing_generations g WHERE g.task_id=NEW.task_id AND g.generation>previous_generation)
        AND NEW.payload=(previous-ARRAY['snapshot_id','confirmation_id','requirement_revision','content_hash','created_at']
            || jsonb_build_object('snapshot_id',NEW.snapshot_id,'confirmation_id',NEW.confirmation_id,
                'requirement_revision',NEW.requirement_revision,'content_hash',NEW.content_hash,
                'created_at',(SELECT c.payload->'created_at' FROM sdlc_confirmations c
                    WHERE c.task_id=NEW.task_id AND c.id=NEW.confirmation_id)))
        AND j.before_state->'lifecycle'->'routing'=previous AND j.after_state->'lifecycle'->'routing'=NEW.payload
        AND j.after_blocker->'resolution'=jsonb_build_object('requirement_revision',NEW.requirement_revision,
            'content_hash',NEW.content_hash,'routing_generation',NEW.generation)
        AND EXISTS(SELECT 1 FROM sdlc_confirmations c WHERE c.task_id=NEW.task_id AND c.id=NEW.confirmation_id
            AND c.revision=NEW.requirement_revision AND c.content_hash=NEW.content_hash
            AND c.payload->>'owner_subject'=j.before_state->>'owner_subject')) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC routing generation requires atomic completion and copied frozen policy' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_critical_pm_generation_gate BEFORE INSERT ON sdlc_task_routing_generations
    FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_generation_gate();

CREATE FUNCTION sdlc_critical_pm_lifecycle_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE op text; valid boolean;
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t JOIN sdlc_delivery_heads d USING(task_id)
        JOIN sdlc_outbox o ON o.task_id=t.task_id AND o.event_id=NEW.outbox_event_id
        WHERE t.task_id=NEW.task_id AND t.state=NEW.after_state AND d.execution=NEW.after_execution
            AND t.pm_execution_id=NEW.execution_id AND t.pm_owner_version=NEW.owner_version
            AND t.pm_admission_state=CASE WHEN NEW.kind='complete' THEN 'completed' ELSE 'reserved' END
            AND o.payload->'result'=NEW.result)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits WHERE task_id=NEW.task_id AND transaction_id=pg_current_xact_id())
        OR EXISTS(SELECT 1 FROM sdlc_pm_state_write_permits WHERE task_id=NEW.task_id AND transaction_id=pg_current_xact_id()) THEN
        RAISE EXCEPTION 'SDLC review operation must commit complete exact projection/outbox without leaked permits' USING ERRCODE='23514';
    END IF;
    IF NEW.kind='claim' THEN
        SELECT EXISTS(SELECT 1 FROM sdlc_pm_lease_operations o WHERE o.execution_id=NEW.execution_id
            AND o.actor_subject=NEW.actor_subject AND o.idempotency_key=NEW.idempotency_key
            AND o.payload_hash=NEW.request_sha256 AND o.lease_version=NEW.lease_version AND o.result=NEW.result) INTO valid;
    ELSIF NEW.kind='resume' THEN
        SELECT EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs o WHERE o.task_id=NEW.task_id AND o.execution_id=NEW.execution_id
            AND o.actor_subject=NEW.actor_subject AND o.idempotency_key=NEW.idempotency_key AND o.transaction_id=NEW.transaction_id
            AND o.payload_hash=NEW.request_sha256 AND o.command=NEW.command AND o.result=NEW.result) INTO valid;
    ELSIF NEW.kind='command' THEN
        SELECT EXISTS(SELECT 1 FROM sdlc_idempotency o WHERE o.task_id=NEW.task_id AND o.actor_subject=NEW.actor_subject
            AND o.idempotency_key=NEW.idempotency_key AND o.payload_hash=NEW.request_sha256 AND o.result=NEW.result) INTO valid;
        op:=NEW.command->>'operation';
        IF op='publish_revision' AND NOT EXISTS(SELECT 1 FROM sdlc_requirements r WHERE r.task_id=NEW.task_id
            AND r.revision=(NEW.result->>'revision')::bigint AND r.payload=NEW.result AND r.content_hash=NEW.result->>'content_hash')
            OR op='publish_question' AND NOT EXISTS(SELECT 1 FROM sdlc_question_versions q WHERE q.task_id=NEW.task_id
                AND q.question_id=(NEW.result->>'id')::uuid AND q.version=(NEW.result->>'version')::bigint AND q.payload=NEW.result)
            OR op='answer' AND NOT EXISTS(SELECT 1 FROM sdlc_answers a WHERE a.task_id=NEW.task_id AND a.id=(NEW.result->>'id')::uuid AND a.payload=NEW.result)
            OR op='evidence' AND NOT EXISTS(SELECT 1 FROM sdlc_evidence e WHERE e.task_id=NEW.task_id AND e.payload=NEW.result)
            OR op='confirm' AND (NOT EXISTS(SELECT 1 FROM sdlc_confirmations c WHERE c.task_id=NEW.task_id
                AND c.id=(NEW.result->>'id')::uuid AND c.payload=NEW.result)
                OR EXISTS(SELECT 1 FROM sdlc_analysis_intents WHERE task_id=NEW.task_id AND confirmation_id=(NEW.result->>'id')::uuid)) THEN
            valid:=false;
        END IF;
    ELSE
        SELECT EXISTS(SELECT 1 FROM sdlc_pm_completions c JOIN sdlc_pm_completion_operations o USING(completion_id)
            JOIN sdlc_task_routing_generations g ON g.completion_id=c.completion_id AND g.operation_id=NEW.operation_id
            JOIN sdlc_analysis_intents i ON i.task_id=g.task_id AND i.review_routing_snapshot_id=g.snapshot_id
            JOIN sdlc_outbox emitted ON emitted.event_id=i.intent_id AND emitted.event_type='analysis.intent_created'
            WHERE c.completion_id=NEW.completion_id AND c.execution_id=NEW.execution_id AND c.result=NEW.result
                AND o.task_id=NEW.task_id AND o.idempotency_key=NEW.idempotency_key AND o.actor_subject=NEW.actor_subject
                AND o.payload_hash=NEW.request_sha256 AND o.result=NEW.result
                AND g.payload=NEW.after_state->'lifecycle'->'routing' AND emitted.payload->'result'=i.payload) INTO valid;
    END IF;
    IF valid IS DISTINCT FROM true THEN RAISE EXCEPTION 'SDLC review actual original ledger/effects missing' USING ERRCODE='23514'; END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_critical_pm_lifecycle_commit_gate AFTER INSERT ON sdlc_critical_pm_lifecycle_operations
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_lifecycle_commit_gate();

CREATE FUNCTION sdlc_critical_pm_lease_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM sdlc_critical_pm_reviews WHERE execution_id=NEW.execution_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_critical_pm_lifecycle_operations j WHERE j.execution_id=NEW.execution_id
            AND j.kind='claim' AND j.transaction_id=pg_current_xact_id() AND j.lease_id=NEW.lease_id
            AND j.lease_version=NEW.version AND j.result=NEW.result) THEN
        RAISE EXCEPTION 'SDLC review claim requires same-transaction canonical journal' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_critical_pm_lease_commit_gate AFTER INSERT ON sdlc_pm_execution_leases
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_lease_commit_gate();

CREATE FUNCTION sdlc_critical_pm_completion_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM sdlc_critical_pm_reviews WHERE execution_id=NEW.execution_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_critical_pm_lifecycle_operations j WHERE j.execution_id=NEW.execution_id
            AND j.kind='complete' AND j.transaction_id=pg_current_xact_id()
            AND j.completion_id=NEW.completion_id AND j.result=NEW.result) THEN
        RAISE EXCEPTION 'SDLC review completion requires same-transaction resolution/generation journal' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_critical_pm_completion_commit_gate AFTER INSERT ON sdlc_pm_completions
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_critical_pm_completion_commit_gate();

CREATE FUNCTION sdlc_critical_pm_exact_event(p_task uuid,p_before jsonb,p_after jsonb)
RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_critical_pm_lifecycle_operations j
        WHERE j.task_id=p_task AND j.transaction_id=pg_current_xact_id() AND j.before_state=p_before AND j.after_state=p_after)
$$;

-- Preserve legacy function bodies; checked anchors insert narrow current-transaction branches only.
DO $$
DECLARE name text; source text; anchor text; replacement text;
BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_delivery_canonical_gate()','sdlc_critical_projection_gate()'] LOOP
        SELECT pg_get_functiondef(to_regprocedure(name)) INTO source;
        IF source IS NULL THEN RAISE EXCEPTION 'SDLC missing guard %',name; END IF;
        anchor:=E'BEGIN\n';
        IF name='sdlc_delivery_canonical_gate()' THEN
            replacement:=E'BEGIN\n    IF sdlc_critical_pm_exact_event(OLD.task_id,OLD.state,NEW.state) AND EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p WHERE p.task_id=OLD.task_id AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN\n        RETURN NEW;\n    END IF;\n';
        ELSE
            replacement:=E'BEGIN\n    IF TG_TABLE_NAME=''sdlc_tasks'' THEN\n        IF sdlc_critical_pm_exact_event(OLD.task_id,OLD.state,NEW.state) THEN RETURN NEW; END IF;\n    END IF;\n    IF EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads h WHERE h.task_id=NEW.task_id AND sdlc_critical_pm_detail(h.blocker_id)->>''state''=''resolved'') THEN RETURN NEW; END IF;\n';
        END IF;
        IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
            RAISE EXCEPTION 'SDLC guard anchor differs %',name;
        END IF;
        EXECUTE replace(source,anchor,replacement);
    END LOOP;
    SELECT pg_get_functiondef('sdlc_pm_resume_input_gate()'::regprocedure) INTO source;
    anchor:='OR EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=NEW.task_id)';
    replacement:='OR (EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=NEW.task_id) AND NOT sdlc_critical_pm_retained_head_allowed(NEW.task_id,NEW.execution_id))';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'SDLC resume overlap anchor differs'; END IF;
    source:=replace(source,anchor,replacement);
    anchor:='t.state->>''stage'' IN (''Backlog'',''Analysis'')';
    replacement:='(t.state->>''stage'' IN (''Backlog'',''Analysis'') OR (t.state->>''stage''=''Draft'' AND t.state->''lifecycle''->>''waiting_reason''=''critical_pm_awaiting_finalization'' AND sdlc_critical_pm_retained_head_allowed(NEW.task_id,NEW.execution_id)))';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'SDLC resume confirmation anchor differs'; END IF;
    EXECUTE replace(source,anchor,replacement);
END $$;

-- Cross-table UUID uniqueness needs the same lock in BOTH insert directions.
-- These are immutable source rows; the view itself is not an authority.
CREATE FUNCTION sdlc_routing_snapshot_collision_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.snapshot_id::text,61014));
    IF (TG_TABLE_NAME='sdlc_task_routing_generations' AND EXISTS(
            SELECT 1 FROM sdlc_task_routing_snapshots WHERE snapshot_id=NEW.snapshot_id))
        OR (TG_TABLE_NAME='sdlc_task_routing_snapshots' AND EXISTS(
            SELECT 1 FROM sdlc_task_routing_generations WHERE snapshot_id=NEW.snapshot_id)) THEN
        RAISE EXCEPTION 'SDLC routing snapshot UUID collision' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_routing_snapshot_collision_gate BEFORE INSERT ON sdlc_task_routing_snapshots
    FOR EACH ROW EXECUTE FUNCTION sdlc_routing_snapshot_collision_gate();
CREATE TRIGGER sdlc_routing_snapshot_collision_gate BEFORE INSERT ON sdlc_task_routing_generations
    FOR EACH ROW EXECUTE FUNCTION sdlc_routing_snapshot_collision_gate();

-- Existing PM ownership/write permits are preserved. This exact journal is the ONLY new branch.
DO $$
DECLARE source text; anchor text; replacement text;
BEGIN
    SELECT pg_get_functiondef('sdlc_pm_reservation_gate()'::regprocedure) INTO source;
    anchor:=E'BEGIN\n';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'SDLC PM ownership guard anchor differs';
    END IF;
    replacement:=E'BEGIN\n    IF sdlc_critical_pm_exact_event(OLD.task_id,OLD.state,NEW.state) THEN\n'
        || E'        IF (NEW.task_id=OLD.task_id AND NEW.root_task_id=OLD.root_task_id AND NEW.project_id=OLD.project_id\n'
        || E'            AND NEW.owner_subject=OLD.owner_subject AND NEW.tracker_instance_id=OLD.tracker_instance_id\n'
        || E'            AND NEW.pm_execution_id=OLD.pm_execution_id AND NEW.pm_owner_version=OLD.pm_owner_version\n'
        || E'            AND NEW.state->''assignment''=OLD.state->''assignment'' AND OLD.pm_admission_state=''reserved''\n'
        || E'            AND NEW.pm_admission_state=CASE WHEN EXISTS(SELECT 1 FROM sdlc_critical_pm_lifecycle_operations j\n'
        || E'                WHERE j.task_id=OLD.task_id AND j.transaction_id=pg_current_xact_id() AND j.kind=''complete''\n'
        || E'                    AND j.before_state=OLD.state AND j.after_state=NEW.state) THEN ''completed'' ELSE ''reserved'' END\n'
        || E'            AND EXISTS(SELECT 1 FROM sdlc_pm_state_write_permits p WHERE p.task_id=OLD.task_id\n'
        || E'                AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state)) IS NOT TRUE THEN\n'
        || E'            RAISE EXCEPTION ''SDLC exact PM review ownership/permit required'' USING ERRCODE=''23514'';\n'
        || E'        END IF;\n        DELETE FROM sdlc_pm_state_write_permits WHERE task_id=OLD.task_id\n'
        || E'            AND transaction_id=pg_current_xact_id() AND next_state=NEW.state;\n        RETURN NEW;\n    END IF;\n';
    EXECUTE replace(source,anchor,replacement);
END $$;

-- Keep every legacy guard condition, changing only the immutable snapshot read projection.
-- NO project-policy lookup or latest-revision relaxation is introduced here.
DO $$
DECLARE name text; source text; anchor text;
BEGIN
    anchor:='FROM sdlc_task_routing_snapshots s';
    FOREACH name IN ARRAY ARRAY['sdlc_draft_flow_gate()','sdlc_delivery_canonical_gate()',
            'sdlc_critical_blocker_insert_gate()','sdlc_critical_pm_return_insert_gate()'] LOOP
        SELECT pg_get_functiondef(to_regprocedure(name)) INTO source;
        IF source IS NULL OR (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
            RAISE EXCEPTION 'SDLC frozen routing read anchor differs %',name;
        END IF;
        EXECUTE replace(source,anchor,'FROM sdlc_routing_snapshot_inventory s');
    END LOOP;
END $$;

-- A subsequent blocker may replace ONLY an actually resolved head; original journals stay immutable.
DO $$
DECLARE source text; anchor text;
BEGIN
    SELECT pg_get_functiondef('sdlc_critical_blocker_insert_gate()'::regprocedure) INTO source;
    anchor:='OR EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads WHERE task_id=NEW.task_id)';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'SDLC new blocker active head anchor differs';
    END IF;
    source:=replace(source,anchor,'OR EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads h WHERE h.task_id=NEW.task_id '
        || 'AND sdlc_critical_pm_detail(h.blocker_id)->>''state'' IS DISTINCT FROM ''resolved'')');
    anchor:='AND s.payload=head->''task''->''routing''';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'SDLC new blocker exact generation anchor differs';
    END IF;
    EXECUTE replace(source,anchor,anchor||' AND to_jsonb(s.generation)=NEW.payload->''routing_generation''');
END $$;

-- Same original closed shape; only the former generation=1 literal becomes bounded.
ALTER TABLE sdlc_critical_blockers DROP CONSTRAINT sdlc_critical_blockers_check;
ALTER TABLE sdlc_critical_blockers ADD CONSTRAINT sdlc_critical_blockers_check CHECK ((
    payload->>'blocker_id'=blocker_id::text AND payload->>'task_id'=task_id::text
    AND payload->>'root_task_id'=task_id::text
    AND payload->'proposal'->'fence'->>'assignment_id'=assignment_id::text
    AND payload->'version'='1'::jsonb AND payload->>'state'='held'
    AND (payload->>'routing_generation')::bigint BETWEEN 1 AND 9007199254740991
    AND payload->>'blocker_sha256' ~ '^[0-9a-f]{64}$'
    AND payload->>'assignment_hash' ~ '^[0-9a-f]{64}$'
    AND payload->'pm_review'='null'::jsonb AND payload->'resolution'='null'::jsonb
    AND payload->'technical_terminal'='null'::jsonb
    AND payload->'proposal'->>'kind' IN ('requirements_contradiction','missing_business_decision')
    AND jsonb_typeof(payload->'proposal'->'options')='array'
    AND jsonb_array_length(payload->'proposal'->'options') BETWEEN 0 AND 8
    AND jsonb_typeof(payload->'proposal'->'evidence')='array'
    AND jsonb_array_length(payload->'proposal'->'evidence') BETWEEN 1 AND 16) IS TRUE);

CREATE FUNCTION sdlc_critical_blocker_head_cas_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE previous jsonb; proposed sdlc_critical_blockers%ROWTYPE; current_head jsonb;
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'SDLC blocker head cannot be deleted' USING ERRCODE='23514'; END IF;
    PERFORM task_id FROM sdlc_tasks WHERE task_id=OLD.task_id FOR UPDATE;
    previous:=sdlc_critical_pm_detail(OLD.blocker_id);
    SELECT * INTO proposed FROM sdlc_critical_blockers WHERE blocker_id=NEW.blocker_id AND task_id=NEW.task_id;
    SELECT receipt INTO current_head FROM sdlc_delivery_tasks WHERE task_id=OLD.task_id;
    IF (NEW.task_id=OLD.task_id AND NEW.blocker_id<>OLD.blocker_id AND previous->>'state'='resolved'
        AND proposed.payload->>'state'='held' AND proposed.previous_receipt=current_head
        AND proposed.payload->'routing_generation'=previous->'resolution'->'routing_generation'
        AND proposed.payload->'routing_snapshot_id'=current_head->'task'->'routing'->'snapshot_id') IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC only resolved blocker head may CAS to actual current technical proposal' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
DROP TRIGGER sdlc_critical_blocker_head_immutable ON sdlc_critical_blocker_heads;
CREATE TRIGGER sdlc_critical_blocker_head_cas_gate BEFORE UPDATE OR DELETE ON sdlc_critical_blocker_heads
    FOR EACH ROW EXECUTE FUNCTION sdlc_critical_blocker_head_cas_gate();

-- Original progress paths remain guarded. Only a real resolved journal removes this hold.
DO $$
DECLARE source text; anchor text;
BEGIN
    SELECT pg_get_functiondef('sdlc_critical_no_progress_gate()'::regprocedure) INTO source;
    anchor:='EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads WHERE task_id=task)';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'SDLC critical progress anchor differs'; END IF;
    EXECUTE replace(source,anchor,'EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads h WHERE h.task_id=task AND sdlc_critical_pm_detail(h.blocker_id)->>''state'' IS DISTINCT FROM ''resolved'')');
END $$;
