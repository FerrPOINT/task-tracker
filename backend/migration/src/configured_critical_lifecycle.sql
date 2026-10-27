-- Preserve native completion foreign keys for historical rows. New business
-- completion references its actual configured ledger, never a fabricated native row.
ALTER TABLE sdlc_critical_pm_lifecycle_operations ADD COLUMN configured_completion_id uuid UNIQUE
    REFERENCES sdlc_configured_pm_completions(completion_id) DEFERRABLE INITIALLY DEFERRED;
DO $$
DECLARE item record; removed integer := 0;
BEGIN
    FOR item IN SELECT conname FROM pg_constraint
        WHERE conrelid='sdlc_critical_pm_lifecycle_operations'::regclass AND contype='c'
            AND position('completion_id IS NOT NULL' IN pg_get_constraintdef(oid))>0
            AND position('kind' IN pg_get_constraintdef(oid))>0
    LOOP
        EXECUTE format('ALTER TABLE sdlc_critical_pm_lifecycle_operations DROP CONSTRAINT %I',item.conname);
        removed:=removed+1;
    END LOOP;
    IF removed<>1 THEN RAISE EXCEPTION 'critical completion family constraint changed'; END IF;
END $$;
ALTER TABLE sdlc_critical_pm_lifecycle_operations ADD CONSTRAINT sdlc_critical_pm_completion_family
    CHECK (num_nonnulls(completion_id,configured_completion_id)=CASE WHEN kind='complete' THEN 1 ELSE 0 END);
ALTER TABLE sdlc_task_routing_generations ALTER COLUMN completion_id DROP NOT NULL,
    ADD COLUMN configured_completion_id uuid UNIQUE
        REFERENCES sdlc_configured_pm_completions(completion_id) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE sdlc_task_routing_generations ADD CONSTRAINT sdlc_critical_pm_generation_completion_family
    CHECK (num_nonnulls(completion_id,configured_completion_id)=1);

-- Forward-only: old operation rows and their actual lease evidence stay intact.
-- Lease columns are absent together only for the configured operation family.
ALTER TABLE sdlc_critical_pm_lifecycle_operations ALTER COLUMN lease_id DROP NOT NULL,
    ALTER COLUMN lease_version DROP NOT NULL;
ALTER TABLE sdlc_critical_pm_lifecycle_operations ADD CONSTRAINT sdlc_critical_pm_operation_lease_pair
    CHECK ((lease_id IS NULL)=(lease_version IS NULL));

CREATE OR REPLACE FUNCTION sdlc_critical_pm_lifecycle_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
<<review_lifecycle>>
DECLARE t sdlc_tasks%ROWTYPE; v sdlc_critical_pm_reviews%ROWTYPE; l sdlc_pm_execution_leases%ROWTYPE;
    head jsonb; detail jsonb; expected jsonb; next_task jsonb; expected_blocker jsonb; body jsonb;
    configured boolean; current_fence jsonb; op text; questions jsonb; q jsonb; prior jsonb; revision jsonb; confirmation jsonb;
BEGIN
    SELECT * INTO t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT * INTO v FROM sdlc_critical_pm_reviews WHERE task_id=t.task_id AND blocker_id=NEW.blocker_id;
    SELECT execution INTO head FROM sdlc_delivery_heads WHERE task_id=t.task_id;
    SELECT * INTO l FROM sdlc_pm_execution_leases WHERE execution_id=NEW.execution_id;
    detail:=sdlc_critical_pm_detail(NEW.blocker_id); revision:=t.state->'revisions'->-1;
    configured:=t.state->>'interaction_protocol'='configured-cli-v1';
    current_fence:=jsonb_build_object('assignment_id',t.state->'assignment'->'assignment_id',
        'execution_id',t.state->'assignment'->'execution_id','agent_id',t.state->'assignment'->'agent_id',
        'assignment_version',t.state->'assignment'->'version');
    -- A configured operation uses the exact persisted PM assignment. Historical
    -- native rows retain their lease proof; no placeholder lease is inserted.
    IF configured IS TRUE AND (NEW.kind NOT IN ('command','complete') OR NEW.lease_id IS NOT NULL
        OR NEW.lease_version IS NOT NULL
        OR v.result->'reservation'->>'interaction_protocol' IS DISTINCT FROM 'configured-cli-v1') THEN
        RAISE EXCEPTION 'configured review operations cannot carry a native lease' USING ERRCODE='23514';
    END IF;

    IF (NEW.transaction_id=pg_current_xact_id() AND t.task_id=t.root_task_id AND t.state->>'flow'='single-task-v1'
        AND t.state->>'stage'='Draft' AND t.pm_admission_state='reserved'
        AND t.pm_execution_id=NEW.execution_id AND v.execution_id=NEW.execution_id
        AND t.pm_owner_version=NEW.owner_version AND v.owner_version=NEW.owner_version
        AND t.state->'assignment'=v.result->'reservation'->'assignment'
        AND NEW.before_state=t.state AND NEW.before_execution=head AND NEW.before_blocker=detail
        AND detail->>'state'='in_pm_review' AND detail->'resolution'='null'::jsonb
        AND (t.state->'lifecycle'->>'version')::bigint BETWEEN 1 AND 9007199254740990
        AND ((configured IS TRUE AND NEW.kind IN ('command','complete') AND NEW.lease_id IS NULL AND NEW.lease_version IS NULL)
            OR (configured IS NOT TRUE AND l.lease_id=NEW.lease_id AND l.version=NEW.lease_version
                AND l.result->'fence'=current_fence))
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
            ((configured IS TRUE AND (NEW.actor_subject IS DISTINCT FROM t.state->'assignment'->>'machine_subject'
                OR t.state->'assignment'->>'machine_subject' IS DISTINCT FROM v.result->'reservation'->'assignment'->>'machine_subject'))
            OR (configured IS NOT TRUE AND (NEW.actor_subject IS DISTINCT FROM l.holder_subject OR l.expires_at<=clock_timestamp()))) THEN
            RAISE EXCEPTION 'SDLC review producer needs actual live current lease' USING ERRCODE='23514';
        END IF;
        IF op='publish_revision' THEN
            IF body->'fence' IS DISTINCT FROM current_fence
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
            IF configured IS TRUE AND (body->'checkpoint_id' IS DISTINCT FROM 'null'::jsonb
                OR NEW.result->'checkpoint_id' IS DISTINCT FROM 'null'::jsonb) THEN
                RAISE EXCEPTION 'configured question has no native checkpoint' USING ERRCODE='23514';
            END IF;
            IF body->'fence' IS DISTINCT FROM current_fence OR body->'requirement_revision' IS DISTINCT FROM revision->'revision'
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
                IF prior->'assignment_id' IS DISTINCT FROM current_fence->'assignment_id'
                    OR prior->'execution_id' IS DISTINCT FROM current_fence->'execution_id'
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
                OR prior->'assignment_id' IS DISTINCT FROM current_fence->'assignment_id'
                OR prior->'execution_id' IS DISTINCT FROM current_fence->'execution_id' THEN
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
                IF NEW.result IS DISTINCT FROM q OR body->'command'->'fence' IS DISTINCT FROM current_fence THEN
                    RAISE EXCEPTION 'SDLC exact review cancellation required' USING ERRCODE='23514';
                END IF;
            END IF;
            SELECT jsonb_agg(CASE WHEN x->'id'=q->'id' THEN q ELSE x END) INTO questions FROM jsonb_array_elements(questions) x;
            expected:=jsonb_set(expected,'{questions}',questions);
            next_task:=jsonb_set(next_task,'{status}',CASE WHEN EXISTS(SELECT 1 FROM jsonb_array_elements(questions) x WHERE x->>'state'='open')
                THEN '"Ready"'::jsonb ELSE '"InProgress"'::jsonb END);
        ELSIF op='evidence' THEN
            IF body->'fence' IS DISTINCT FROM current_fence
                OR NEW.result-'created_at'-'verifier_subject' IS DISTINCT FROM body-'fence'-'idempotency_key'
                OR NEW.result->>'verifier_subject' IS DISTINCT FROM NEW.actor_subject
                OR NEW.result->'requirement_revision' IS DISTINCT FROM revision->'revision'
                OR NEW.result->'content_hash' IS DISTINCT FROM revision->'content_hash' THEN
                RAISE EXCEPTION 'SDLC actual review evidence required' USING ERRCODE='23514';
            END IF;
            expected:=jsonb_set(expected,'{evidence}',(expected->'evidence')||jsonb_build_array(NEW.result));
        ELSIF op='confirm' THEN
            IF NEW.actor_subject IS DISTINCT FROM t.owner_subject OR NEW.result->>'owner_subject' IS DISTINCT FROM t.owner_subject
                OR NEW.result->>'stage' IS DISTINCT FROM CASE WHEN configured IS TRUE THEN 'Draft' ELSE 'Backlog' END OR NEW.result->>'task_id' IS DISTINCT FROM t.task_id::text
                OR NEW.result->'revision' IS DISTINCT FROM revision->'revision'
                OR NEW.result->'content_hash' IS DISTINCT FROM revision->'content_hash'
                OR body->'revision' IS DISTINCT FROM revision->'revision'
                OR body->'command'->'content_hash' IS DISTINCT FROM revision->'content_hash'
                OR (revision->>'revision')::bigint <= (v.result->'context'->'requirements'->>'revision')::bigint
                OR EXISTS(SELECT 1 FROM sdlc_confirmations c WHERE c.task_id=t.task_id AND c.revision=(NEW.result->>'revision')::bigint) THEN
                RAISE EXCEPTION 'SDLC new exact owner confirmation required' USING ERRCODE='23514';
            END IF;
            expected:=jsonb_set(expected,'{confirmations}',(expected->'confirmations')||jsonb_build_array(NEW.result));
            expected:=jsonb_set(expected,'{draft_submode}','"awaiting_confirmation"'::jsonb);
            next_task:=jsonb_set(next_task,'{status}','"Ready"'::jsonb);
            next_task:=jsonb_set(next_task,'{waiting_reason}','"critical_pm_awaiting_finalization"'::jsonb);
            IF configured IS TRUE THEN
                next_task:=jsonb_set(next_task,'{status}','"InProgress"'::jsonb);
            END IF;
        ELSE RAISE EXCEPTION 'SDLC unsupported review command' USING ERRCODE='23514'; END IF;
    ELSIF configured IS TRUE THEN
        SELECT c.payload INTO confirmation FROM sdlc_confirmations c WHERE c.task_id=t.task_id
            AND c.revision=(review_lifecycle.revision->>'revision')::bigint
            AND c.content_hash=review_lifecycle.revision->>'content_hash';
        IF NEW.completion_id IS NOT NULL OR NEW.configured_completion_id IS NULL
            OR NEW.actor_subject IS DISTINCT FROM t.state->'assignment'->>'machine_subject'
            OR confirmation IS NULL OR confirmation->>'owner_subject' IS DISTINCT FROM t.owner_subject
            OR t.state->'confirmation_revision' IS DISTINCT FROM revision->'revision'
            OR t.state->'lifecycle'->>'waiting_reason' IS DISTINCT FROM 'critical_pm_awaiting_finalization'
            OR (revision->>'revision')::bigint <= (v.result->'context'->'requirements'->>'revision')::bigint
            OR NOT EXISTS(SELECT 1 FROM sdlc_configured_pm_completions c
                WHERE c.completion_id=NEW.configured_completion_id AND c.task_id=t.task_id
                    AND c.execution_id=NEW.execution_id AND c.actor_subject=NEW.actor_subject
                    AND c.idempotency_key=NEW.idempotency_key AND c.payload_hash=NEW.request_sha256
                    AND c.command=NEW.command AND c.result=NEW.result
                    AND c.requirement_revision=(revision->>'revision')::bigint
                    AND c.content_hash=revision->>'content_hash' AND c.confirmation_id=(confirmation->>'id')::uuid) THEN
            RAISE EXCEPTION 'configured review requires actual saved business completion and owner consent' USING ERRCODE='23514';
        END IF;
        expected_blocker:=jsonb_set(detail,'{version}',to_jsonb((detail->>'version')::bigint+1));
        expected_blocker:=jsonb_set(expected_blocker,'{state}','"resolved"'::jsonb);
        expected_blocker:=jsonb_set(expected_blocker,'{resolution}',jsonb_build_object('requirement_revision',revision->'revision',
            'content_hash',revision->'content_hash','routing_generation',(detail->>'routing_generation')::bigint+1));
        next_task:=jsonb_set(next_task,'{stage}','"Backlog"'::jsonb);
        next_task:=jsonb_set(next_task,'{status}','"Ready"'::jsonb);
        next_task:=jsonb_set(next_task,'{runtime}','"terminal"'::jsonb);
        next_task:=jsonb_set(next_task,'{waiting_reason}','null'::jsonb);
        next_task:=jsonb_set(next_task,'{cycle}','0'::jsonb);
        next_task:=jsonb_set(next_task,'{transient_retries}','0'::jsonb);
        next_task:=jsonb_set(next_task,'{routing}',NEW.after_state->'lifecycle'->'routing');
        expected:=expected-'draft_submode';
    ELSE
        IF NEW.configured_completion_id IS NOT NULL THEN
            RAISE EXCEPTION 'historical review cannot use configured completion' USING ERRCODE='23514';
        END IF;
        IF EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE execution_id=NEW.execution_id) THEN
            RAISE EXCEPTION 'SDLC current review completion already exists' USING ERRCODE='23514';
        END IF;
        SELECT c.payload INTO confirmation FROM sdlc_confirmations c WHERE c.task_id=t.task_id
            AND c.revision=(review_lifecycle.revision->>'revision')::bigint
            AND c.content_hash=review_lifecycle.revision->>'content_hash';
        IF confirmation IS NULL OR confirmation->>'owner_subject' IS DISTINCT FROM t.owner_subject
            OR t.state->'confirmation_revision' IS DISTINCT FROM revision->'revision'
            OR t.state->'lifecycle'->>'waiting_reason' IS DISTINCT FROM 'critical_pm_awaiting_finalization'
            OR (revision->>'revision')::bigint <= (v.result->'context'->'requirements'->>'revision')::bigint
        OR NEW.command IS DISTINCT FROM NEW.result-ARRAY['completion_id','tracker_instance_id','project_id','task_id',
                'root_task_id','owner_subject','owner_version','previous_pm_admission_state','next_pm_admission_state','created_at']
            OR NEW.result->>'completion_id' IS DISTINCT FROM NEW.completion_id::text
            OR NEW.result->'fence' IS DISTINCT FROM current_fence OR NEW.result->>'lease_id' IS DISTINCT FROM l.lease_id::text
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

CREATE OR REPLACE FUNCTION sdlc_critical_pm_generation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE j sdlc_critical_pm_lifecycle_operations%ROWTYPE; previous jsonb; previous_generation bigint;
BEGIN
    SELECT * INTO j FROM sdlc_critical_pm_lifecycle_operations WHERE operation_id=NEW.operation_id;
    SELECT payload,generation INTO previous,previous_generation FROM sdlc_routing_snapshot_inventory
        WHERE task_id=NEW.task_id AND snapshot_id=NEW.previous_snapshot_id;
    IF (j.kind='complete' AND j.transaction_id=pg_current_xact_id() AND j.task_id=NEW.task_id
        AND j.blocker_id=NEW.blocker_id AND j.execution_id=NEW.execution_id AND j.completion_id IS NOT DISTINCT FROM NEW.completion_id
        AND j.configured_completion_id IS NOT DISTINCT FROM NEW.configured_completion_id
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

CREATE OR REPLACE FUNCTION sdlc_critical_pm_lifecycle_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
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
    ELSIF NEW.configured_completion_id IS NOT NULL THEN
        SELECT EXISTS(SELECT 1 FROM sdlc_configured_pm_completions c
            JOIN sdlc_task_routing_generations g ON g.configured_completion_id=c.completion_id AND g.operation_id=NEW.operation_id
            WHERE c.completion_id=NEW.configured_completion_id AND c.execution_id=NEW.execution_id
                AND c.task_id=NEW.task_id AND c.result=NEW.result AND c.command=NEW.command
                AND c.idempotency_key=NEW.idempotency_key AND c.actor_subject=NEW.actor_subject
                AND c.payload_hash=NEW.request_sha256 AND g.payload=NEW.after_state->'lifecycle'->'routing'
                AND NEW.after_state->>'stage'='Backlog' AND NEW.after_state->>'status'='Ready'
                AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents i WHERE i.task_id=g.task_id
                    AND i.confirmation_id=g.confirmation_id)) INTO valid;
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

CREATE OR REPLACE FUNCTION sdlc_configured_cli_completion_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state->>'interaction_protocol' IS DISTINCT FROM 'configured-cli-v1' THEN RETURN NEW; END IF;
    IF TG_OP='UPDATE' AND sdlc_critical_pm_exact_event(OLD.task_id,OLD.state,NEW.state)
        AND NEW.state->>'stage'='Backlog' AND NEW.state->>'status'='Ready'
        AND sdlc_current_configured_pm_completed(NEW.task_id,NEW.pm_execution_id,NEW.state,NEW.pm_admission_state)
        AND EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p WHERE p.task_id=NEW.task_id
            AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN
        RETURN NEW;
    END IF;
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

CREATE OR REPLACE FUNCTION sdlc_configured_cli_completion_committed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=NEW.task_id
        AND t.state->>'stage'='Backlog' AND t.state->>'status'='Ready'
        AND sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)) THEN
        RAISE EXCEPTION 'configured completion must atomically publish Backlog' USING ERRCODE='23514';
    END IF;
    IF EXISTS(SELECT 1 FROM sdlc_critical_pm_reviews WHERE execution_id=NEW.execution_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_critical_pm_lifecycle_operations j
            WHERE j.execution_id=NEW.execution_id AND j.kind='complete'
                AND j.transaction_id=pg_current_xact_id()
                AND j.configured_completion_id=NEW.completion_id AND j.result=NEW.result) THEN
        RAISE EXCEPTION 'configured review completion requires atomic resolution and routing generation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
