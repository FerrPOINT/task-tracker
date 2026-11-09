-- Existing creation rows remain unchanged. The original Draft receipt describes
-- creation; the separate human intake receipt describes the requested handoff.
ALTER TABLE sdlc_draft_creations ADD COLUMN finding_intake jsonb,
    ADD COLUMN finding_intake_result jsonb, ADD COLUMN intake_transaction_id xid8, ADD COLUMN finding_workflow_ordinal bigint UNIQUE;
ALTER TABLE sdlc_draft_creations ADD CONSTRAINT finding_intake_shape CHECK (
    ((finding_intake IS NULL AND finding_intake_result IS NULL AND intake_transaction_id IS NULL AND finding_workflow_ordinal IS NULL) OR
    (jsonb_typeof(finding_intake)='object' AND jsonb_typeof(finding_intake_result)='object'
        AND intake_transaction_id IS NOT NULL
        AND finding_intake ?& ARRAY['source','destination','kind','title','description','expected','criteria','evidence','custom_fields']
        AND finding_intake-ARRAY['source','destination','kind','title','description','expected','criteria','evidence','custom_fields']='{}'::jsonb
        AND finding_intake->'source'->>'application'='fleet-control'
        AND (finding_intake->'source')-ARRAY['application','finding_id','finding_revision']='{}'::jsonb
        AND finding_intake->'source'->>'finding_id' ~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
        AND finding_intake->'source'->>'finding_id'<>'00000000-0000-0000-0000-000000000000'
        AND (finding_intake->'source'->>'finding_revision')::numeric BETWEEN 1 AND 9007199254740991
        AND ((finding_intake->>'destination'='Draft' AND finding_workflow_ordinal IS NULL) OR (finding_intake->>'destination'='Backlog' AND finding_workflow_ordinal>0))
        AND finding_intake->>'destination' IN ('Draft','Backlog') AND finding_intake->>'kind' IN ('bug','refactoring')
        AND finding_intake->>'title'=input_title AND jsonb_typeof(finding_intake->'custom_fields')='object'
        AND finding_intake_result-ARRAY['operation_id','tracker_instance_id','project_id','task_id','task_key','destination','request']='{}'::jsonb
        AND finding_intake_result->>'operation_id' ~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
        AND idempotency_key='fleet-background:'||(finding_intake_result->>'operation_id')
        AND finding_intake_result->'tracker_instance_id'=result->'tracker_instance_id'
        AND finding_intake_result->'project_id'=result->'project_id'
        AND finding_intake_result->'task_id'=result->'task_id'
        AND finding_intake_result->'task_key'=result->'task_key'
        AND finding_intake_result->'destination'=finding_intake->'destination' AND finding_intake_result->'request'=finding_intake)) IS TRUE);

-- A finding handoff is genuine human approval of the exact material below. It
-- grants no PM completion, machine grant, assignment or native execution.
CREATE FUNCTION sdlc_finding_intake_origin(task uuid, state jsonb) RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM sdlc_draft_creations c JOIN sdlc_tasks t ON t.task_id=c.task_id
        JOIN issues i ON i.id=t.task_id AND i.project_id=t.project_id
        JOIN users u ON u.id=i.reporter_id AND u.central_sub=t.owner_subject AND u.is_active
        WHERE c.task_id=$1 AND c.finding_intake->>'destination'='Backlog'
        AND c.actor_subject=t.owner_subject AND c.project_id=t.project_id AND t.root_task_id=t.task_id
        AND i.deleted_at IS NULL AND t.pm_execution_id IS NULL AND t.state->'assignment'='null'::jsonb
        AND $2->>'finding_intake_operation'=c.finding_intake_result->>'operation_id'
        AND c.finding_workflow_ordinal>0 AND $2->>'workflow_task_key'='SDLC-'||c.finding_workflow_ordinal::text
        AND $2->>'interaction_protocol'='configured-cli-v1' AND $2->>'flow'='single-task-v1'
        AND $2->>'owner_subject'=c.actor_subject AND $2->'confirmation_revision'='1'::jsonb
        AND jsonb_array_length($2->'revisions')=1 AND jsonb_array_length($2->'confirmations')=1
        AND $2->'questions'='[]'::jsonb AND $2->'revisions'->0->'revision'='1'::jsonb
        AND $2->'revisions'->0->>'author_subject'=c.actor_subject
        AND ($2->'revisions'->0)-ARRAY['revision','content_hash','author_subject','created_at']
            =jsonb_build_object('goal',c.finding_intake->'title','scope',jsonb_build_array(c.finding_intake->'description'),
                'scenarios',jsonb_build_array(c.finding_intake->'expected'),
                'acceptance_criteria',jsonb_build_array(c.finding_intake->'criteria'),
                'prerequisites',jsonb_build_array('finding:'||(c.finding_intake->'source'->>'finding_id')||':'||(c.finding_intake->'source'->>'finding_revision')||':evidence'),
                'exclusions','[]'::jsonb,'constraints','[]'::jsonb,'dependencies',jsonb_build_array(c.finding_intake->'evidence'),
                'assumptions','[]'::jsonb,'checklist',jsonb_build_array('finding:'||(c.finding_intake->'source'->>'finding_id')||':'||(c.finding_intake->'source'->>'finding_revision')||':acceptance'))
        AND $2->'confirmations'->0->>'owner_subject'=c.actor_subject
        AND $2->'confirmations'->0->>'stage'='Draft'
        AND $2->'confirmations'->0->>'task_id'=t.task_id::text
        AND $2->'confirmations'->0->'revision'='1'::jsonb
        AND $2->'confirmations'->0->'content_hash'=$2->'revisions'->0->'content_hash')
$$;

CREATE FUNCTION sdlc_finding_marker_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state->'finding_intake_operation' IS DISTINCT FROM NEW.state->'finding_intake_operation' THEN
        IF NOT ((NOT OLD.state ? 'finding_intake_operation' AND sdlc_finding_intake_origin(NEW.task_id,NEW.state)
            AND OLD.state->>'stage'='Draft' AND OLD.state->>'status'='Ready'
            AND OLD.state->'revisions'='[]'::jsonb AND OLD.state->'confirmations'='[]'::jsonb
            AND NOT OLD.state ? 'lifecycle' AND NOT NEW.state ? 'lifecycle'
            AND OLD.state-ARRAY['revisions','confirmations','confirmation_revision','workflow_task_key','finding_intake_operation']
                =NEW.state-ARRAY['revisions','confirmations','confirmation_revision','workflow_task_key','finding_intake_operation']
            AND EXISTS(SELECT 1 FROM sdlc_draft_creations c WHERE c.task_id=NEW.task_id
                AND c.intake_transaction_id=pg_current_xact_id())) IS TRUE) THEN
            RAISE EXCEPTION 'SDLC human finding origin is immutable' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_finding_marker_gate BEFORE UPDATE ON sdlc_tasks FOR EACH ROW EXECUTE FUNCTION sdlc_finding_marker_gate();

ALTER TABLE sdlc_delivery_operations DROP CONSTRAINT sdlc_delivery_start_command;
ALTER TABLE sdlc_delivery_operations ADD CONSTRAINT sdlc_delivery_start_command CHECK (
    ((command IS NULL AND transaction_id IS NULL) OR
    (command IS NOT NULL AND transaction_id IS NOT NULL AND event_id IS NOT NULL
        AND jsonb_typeof(command->'idempotency_key')='string' AND command->>'idempotency_key'=idempotency_key
        AND ((command->>'operation'='start_analysis'
            AND command-ARRAY['operation','pm_completion_id','idempotency_key']='{}'::jsonb
            AND command->>'pm_completion_id' ~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
            AND command->>'pm_completion_id'<>'00000000-0000-0000-0000-000000000000')
        OR (command->>'operation'='start_finding_analysis'
            AND command-ARRAY['operation','intake_operation_id','idempotency_key']='{}'::jsonb
            AND command->>'intake_operation_id' ~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
            AND idempotency_key='finding-analysis:'||(command->>'intake_operation_id'))))) IS TRUE);

CREATE FUNCTION sdlc_finding_start_insert_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE t sdlc_tasks; r sdlc_task_routing_snapshots; expected jsonb;
BEGIN
    IF NEW.command->>'operation' IS DISTINCT FROM 'start_finding_analysis' THEN RETURN NEW; END IF;
    SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT * INTO r FROM sdlc_task_routing_snapshots WHERE task_id=t.task_id;
    expected:=jsonb_build_object('task',jsonb_build_object('task_id',t.task_id,'owner_subject',t.owner_subject,
        'version',1,'stage','Analysis','status','Ready','cycle',0,'transient_retries',0,
        'waiting_reason','queued_for_assignment','runtime','queued','routing',r.payload,'previous_result_id',NULL),
        'assignment',NULL,'lease',NULL,'binding',NULL,'result',NULL);
    IF NOT ((sdlc_finding_intake_origin(t.task_id,t.state)
        AND t.state->>'stage'='Draft' AND t.state->>'status'='Ready' AND NOT t.state ? 'lifecycle'
        AND NEW.actor_subject=t.owner_subject AND NEW.transaction_id=pg_current_xact_id()
        AND NEW.command->>'intake_operation_id'=t.state->>'finding_intake_operation'
        AND EXISTS(SELECT 1 FROM sdlc_draft_creations c WHERE c.task_id=t.task_id AND c.intake_transaction_id=NEW.transaction_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=t.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=t.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=t.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents WHERE task_id=t.task_id)
        AND r.payload->'contract_version'='2'::jsonb
        AND r.confirmation_id::text=t.state->'confirmations'->0->>'id'
        AND EXISTS(SELECT 1 FROM sdlc_task_source_bindings b WHERE b.task_id=t.task_id
            AND b.routing_snapshot_id=r.snapshot_id AND b.confirmation_id=r.confirmation_id
            AND b.requirement_revision=r.requirement_revision AND b.content_hash=r.content_hash)
        AND NEW.result=expected) IS TRUE) THEN
        RAISE EXCEPTION 'SDLC finding start requires exact human intake/source/routing' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_finding_start_insert_gate BEFORE INSERT ON sdlc_delivery_operations
FOR EACH ROW EXECUTE FUNCTION sdlc_finding_start_insert_gate();

-- Extend checked boundaries; every existing PM and review branch stays intact.
DO $patch$
DECLARE source text; anchor text; replacement text;
BEGIN
    source:=replace(pg_get_functiondef('sdlc_configured_cli_confirmation_gate()'::regprocedure),E'\r\n',E'\n');
    anchor:=$a$    IF NOT ((t.state->>'stage'='Draft' AND t.pm_admission_state='reserved'$a$;
    replacement:=$b$    IF sdlc_finding_intake_origin(t.task_id,t.state) AND t.state->>'stage'='Draft'
        AND t.state->>'status'='Ready' AND NEW.payload=t.state->'confirmations'->0
        AND NEW.revision=1 AND NEW.content_hash=t.state->'revisions'->0->>'content_hash'
        AND NEW.payload->>'id'=NEW.id::text THEN RETURN NEW; END IF;
    IF NOT ((t.state->>'stage'='Draft' AND t.pm_admission_state='reserved'$b$;
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'finding consent anchor changed'; END IF;
    EXECUTE replace(source,anchor,replacement);

    source:=replace(pg_get_functiondef('sdlc_configured_start_insert_gate()'::regprocedure),E'\r\n',E'\n');
    anchor:='IF NEW.command IS NULL THEN RETURN NEW; END IF;';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'finding start insert anchor changed'; END IF;
    EXECUTE replace(source,anchor,'IF NEW.command IS NULL OR NEW.command->>''operation''=''start_finding_analysis'' THEN RETURN NEW; END IF;');

    source:=replace(pg_get_functiondef('sdlc_configured_start_exact(uuid,jsonb,jsonb)'::regprocedure),E'\r\n',E'\n');
    anchor:=$a$o.command->>'operation'='start_analysis'$a$;
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'finding exact command anchor changed'; END IF;
    source:=replace(source,anchor,$b$(o.command->>'operation'='start_analysis' OR (o.command->>'operation'='start_finding_analysis' AND sdlc_finding_intake_origin($1,$2)))$b$);
    anchor:=$a$$2->>'stage'='Draft' AND $2->>'status'='Completed'$a$;
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'finding exact status anchor changed'; END IF;
    EXECUTE replace(source,anchor,$b$$2->>'stage'='Draft' AND ($2->>'status'='Completed' OR (o.command->>'operation'='start_finding_analysis' AND $2->>'status'='Ready'))$b$);

    source:=replace(pg_get_functiondef('sdlc_configured_cli_completion_transition()'::regprocedure),E'\r\n',E'\n');
    anchor:=E'BEGIN\n';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'finding transition anchor changed'; END IF;
    EXECUTE replace(source,anchor,anchor||E'    IF TG_OP=''UPDATE'' AND sdlc_configured_start_exact(OLD.task_id,OLD.state,NEW.state) THEN RETURN NEW; END IF;\n');

    source:=replace(pg_get_functiondef('sdlc_configured_start_committed()'::regprocedure),E'\r\n',E'\n');
    anchor:='AND sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)';
    IF (length(source)-length(replace(source,anchor,'')))/length(anchor)<>1 THEN RAISE EXCEPTION 'finding commit authority anchor changed'; END IF;
    EXECUTE replace(source,anchor,'AND (sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state) OR (NEW.command->>''operation''=''start_finding_analysis'' AND sdlc_finding_intake_origin(t.task_id,t.state)))');
END $patch$;

CREATE FUNCTION sdlc_finding_intake_commit_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.finding_intake IS NULL THEN RETURN NEW; END IF;
    IF NEW.intake_transaction_id<>pg_current_xact_id() OR NOT EXISTS(SELECT 1 FROM issues i WHERE i.id=NEW.task_id
        AND i.issue_type=CASE WHEN NEW.finding_intake->>'kind'='bug' THEN 'bug' ELSE 'task' END) THEN
        RAISE EXCEPTION 'SDLC finding intake creation type/transaction mismatch' USING ERRCODE='23514';
    END IF;
    IF NEW.finding_intake->>'destination'='Backlog' AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_operations o
        JOIN sdlc_tasks t ON t.task_id=o.task_id WHERE o.task_id=NEW.task_id
        AND o.command->>'operation'='start_finding_analysis' AND o.transaction_id=NEW.intake_transaction_id
        AND o.command->>'intake_operation_id'=NEW.finding_intake_result->>'operation_id'
        AND t.state->'lifecycle'=o.result->'task' AND sdlc_finding_intake_origin(t.task_id,t.state)) THEN
        RAISE EXCEPTION 'SDLC finding Backlog intake requires committed analysis queue' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_finding_intake_commit_gate AFTER INSERT ON sdlc_draft_creations
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_finding_intake_commit_gate();
