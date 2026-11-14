-- Runtime routing may advance only for explicitly resumed, closed, unassigned work.
-- Existing critical-PM generations and all historical rows remain unchanged.
ALTER TABLE sdlc_task_routing_generations
    ALTER COLUMN blocker_id DROP NOT NULL, ALTER COLUMN execution_id DROP NOT NULL,
    ALTER COLUMN operation_id DROP NOT NULL, ADD COLUMN resume jsonb;
ALTER TABLE sdlc_task_routing_generations
    DROP CONSTRAINT sdlc_critical_pm_generation_completion_family;
ALTER TABLE sdlc_task_routing_generations ADD CONSTRAINT sdlc_routing_generation_owner_family CHECK (
    (resume IS NULL AND blocker_id IS NOT NULL AND execution_id IS NOT NULL AND operation_id IS NOT NULL
        AND num_nonnulls(completion_id,configured_completion_id)=1)
    OR (resume IS NOT NULL AND num_nonnulls(blocker_id,execution_id,operation_id,completion_id,configured_completion_id)=0)
);
-- A new runtime generation can retain the SAME owner confirmation.
DO $$
DECLARE item record; removed integer:=0;
BEGIN
    FOR item IN SELECT conname FROM pg_constraint WHERE conrelid='sdlc_task_routing_generations'::regclass
        AND contype='u' AND conkey=ARRAY[(SELECT attnum FROM pg_attribute
            WHERE attrelid='sdlc_task_routing_generations'::regclass AND attname='confirmation_id')]::smallint[]
    LOOP
        EXECUTE format('ALTER TABLE sdlc_task_routing_generations DROP CONSTRAINT %I',item.conname);
        removed:=removed+1;
    END LOOP;
    IF removed<>1 THEN RAISE EXCEPTION 'SDLC routing confirmation uniqueness changed'; END IF;
END $$;

CREATE FUNCTION sdlc_routing_resume_result(g sdlc_task_routing_generations) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE next_task jsonb; role text;
BEGIN
    next_task:=g.resume->'before_receipt'->'task';
    role:=CASE next_task->>'stage' WHEN 'Analysis' THEN 'analyst' WHEN 'Architecture' THEN 'architect'
        WHEN 'Development' THEN 'developer' WHEN 'Rework' THEN 'developer' WHEN 'Review' THEN 'reviewer'
        WHEN 'Testing' THEN 'tester' WHEN 'Deployment' THEN 'devops' END;
    IF next_task#>>ARRAY['routing','policy','routes',role,'agent_id'] IS DISTINCT FROM
        g.payload#>>ARRAY['policy','routes',role,'agent_id'] THEN
        next_task:=next_task-'resume_chat_id';
    END IF;
    next_task:=next_task || jsonb_build_object('version',(next_task->>'version')::bigint+1,
        'runtime','queued','waiting_reason','queued_after_intervention','transient_retries',0,'routing',g.payload);
    RETURN jsonb_set(g.resume->'before_receipt','{task}',next_task);
END $$;

CREATE FUNCTION sdlc_routing_resume_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks; head jsonb; previous jsonb; previous_generation bigint; body jsonb; policy jsonb;
BEGIN
    IF NEW.resume IS NULL THEN RETURN NEW; END IF;
    SELECT * INTO t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT receipt INTO head FROM sdlc_delivery_tasks WHERE task_id=NEW.task_id;
    SELECT payload,generation INTO previous,previous_generation FROM sdlc_routing_snapshot_inventory
        WHERE task_id=NEW.task_id AND snapshot_id=NEW.previous_snapshot_id;
    SELECT r.payload INTO policy FROM sdlc_project_routing_revisions r
        JOIN sdlc_project_routing_heads h USING(project_id)
        WHERE r.project_id=NEW.project_id AND r.version=NEW.policy_version AND h.version=r.version;
    body:=NEW.resume->'command';
    IF (jsonb_typeof(NEW.resume)='object'
        AND NEW.resume ?& ARRAY['actor_subject','idempotency_key','payload_hash','command','before_receipt','transaction_id']
        AND NEW.resume-ARRAY['actor_subject','idempotency_key','payload_hash','command','before_receipt','transaction_id']='{}'::jsonb
        AND NEW.resume->>'transaction_id'=pg_current_xact_id()::text
        AND length(NEW.resume->>'actor_subject')>0 AND NEW.resume->>'payload_hash' ~ '^[0-9a-f]{64}$'
        AND octet_length(NEW.resume->>'idempotency_key') BETWEEN 1 AND 128
        AND body=jsonb_build_object('operation','resume_with_current_routing',
            'expected_task_version',head->'task'->'version','expected_routing_policy_version',NEW.policy_version,
            'idempotency_key',NEW.resume->>'idempotency_key')
        AND t.task_id=t.root_task_id AND t.project_id=NEW.project_id
        AND t.state->>'flow'='single-task-v1' AND t.state->>'interaction_protocol'='configured-cli-v1'
        AND t.state->'lifecycle'=head->'task' AND head->'task'->'routing'=previous
        AND head=NEW.resume->'before_receipt' AND head->'task'->>'status'='Ready'
        AND head->'task'->>'runtime'='terminal'
        AND head->'task'->>'stage' IN ('Analysis','Architecture','Development','Review','Testing','Rework','Deployment')
        AND head-ARRAY['task']=jsonb_build_object('assignment',NULL,'lease',NULL,'binding',NULL,'result',NULL)
        AND (head->'task'->>'version')::bigint BETWEEN 1 AND 9007199254740990
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=NEW.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=NEW.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads h WHERE h.task_id=NEW.task_id
            AND sdlc_critical_pm_detail(h.blocker_id)->>'state' IS DISTINCT FROM 'resolved')
        AND sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)
        AND NEW.generation=previous_generation+1
        AND NOT EXISTS(SELECT 1 FROM sdlc_task_routing_generations g WHERE g.task_id=NEW.task_id AND g.generation>previous_generation)
        AND NEW.payload=(previous-ARRAY['snapshot_id','policy','created_at']
            || jsonb_build_object('snapshot_id',NEW.snapshot_id,'policy',policy,'created_at',NEW.payload->'created_at'))
        AND NEW.payload->>'tracker_instance_id'=t.tracker_instance_id
        AND NEW.confirmation_id=(previous->>'confirmation_id')::uuid
        AND NEW.requirement_revision=(previous->>'requirement_revision')::bigint
        AND NEW.content_hash=previous->>'content_hash' AND policy->'contract_version'='2'::jsonb
        AND EXISTS(SELECT 1 FROM sdlc_confirmations c WHERE c.task_id=NEW.task_id AND c.id=NEW.confirmation_id
            AND c.revision=NEW.requirement_revision AND c.content_hash=NEW.content_hash
            AND c.payload->>'owner_subject'=t.owner_subject
            AND c.revision=(t.state->'revisions'->-1->>'revision')::bigint
            AND c.content_hash=t.state->'revisions'->-1->>'content_hash')) IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC routing resume requires exact closed custody, current policy and original consent' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_a_routing_resume_insert_gate BEFORE INSERT ON sdlc_task_routing_generations
    FOR EACH ROW EXECUTE FUNCTION sdlc_routing_resume_insert_gate();

CREATE FUNCTION sdlc_routing_resume_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE expected jsonb;
BEGIN
    IF NEW.resume IS NULL THEN RETURN NEW; END IF;
    expected:=sdlc_routing_resume_result(NEW);
    IF NOT EXISTS(SELECT 1 FROM sdlc_delivery_operations o JOIN sdlc_outbox e ON e.event_id=o.event_id
        JOIN sdlc_delivery_tasks d ON d.task_id=o.task_id
        WHERE o.task_id=NEW.task_id AND o.actor_subject=NEW.resume->>'actor_subject'
            AND o.idempotency_key=NEW.resume->>'idempotency_key' AND o.payload_hash=NEW.resume->>'payload_hash'
            AND o.result=expected AND d.receipt=expected AND e.task_id=NEW.task_id
            AND e.event_type='delivery.queued' AND e.payload->'result'=expected) THEN
        RAISE EXCEPTION 'SDLC resumed routing requires its atomic original lifecycle operation and event' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_routing_resume_commit_gate AFTER INSERT ON sdlc_task_routing_generations
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_routing_resume_commit_gate();

-- Preserve the complete existing critical-generation branch; only the new,
-- separately validated resume family takes this return.
DO $$
DECLARE source text; anchor text;
BEGIN
    SELECT pg_get_functiondef('sdlc_critical_pm_generation_gate()'::regprocedure) INTO source;
    source:=replace(source,E'\r\n',E'\n'); anchor:=E'BEGIN\n';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'SDLC critical generation guard anchor changed'; END IF;
    EXECUTE replace(source,anchor,anchor||E'    IF NEW.resume IS NOT NULL THEN RETURN NEW; END IF;\n');
    SELECT pg_get_functiondef('sdlc_delivery_canonical_gate()'::regprocedure) INTO source;
    source:=replace(source,E'\r\n',E'\n'); anchor:='        IF old_task IS NULL THEN';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'SDLC canonical routing guard anchor changed'; END IF;
    EXECUTE replace(source,anchor,$resume$
        IF EXISTS(SELECT 1 FROM sdlc_task_routing_generations g WHERE g.task_id=OLD.task_id AND g.resume IS NOT NULL
            AND g.resume->>'transaction_id'=pg_current_xact_id()::text
            AND g.resume->'before_receipt'->'task'=old_task
            AND sdlc_routing_resume_result(g)->'task'=next_task
            AND NEW.state=(OLD.state-'draft_submode' || jsonb_build_object('stage',next_task->'stage',
                'status',next_task->'status','waiting_reason',next_task->'waiting_reason',
                'runtime_state',next_task->'runtime','lifecycle',next_task))) THEN
            RETURN NEW;
        END IF;
$resume$||anchor);
    -- A resolved blocker remains historical when a later explicit runtime resume
    -- advances routing. A new blocker must attest the current inventory generation.
    SELECT pg_get_functiondef('sdlc_critical_blocker_head_cas_gate()'::regprocedure) INTO source;
    anchor:='proposed.payload->''routing_generation''=previous->''resolution''->''routing_generation''';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'SDLC resolved blocker generation guard anchor changed'; END IF;
    EXECUTE replace(source,anchor,'proposed.payload->''routing_generation''=(SELECT to_jsonb(s.generation)
        FROM sdlc_routing_snapshot_inventory s WHERE s.task_id=OLD.task_id AND s.payload=current_head->''task''->''routing'')');
END $$;
