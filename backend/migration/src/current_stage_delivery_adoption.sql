-- Genuine human entry reuses the existing technical assignment, capacity and lease ledger.
CREATE TABLE task_handoff_delivery_adoptions (
 handoff_id uuid PRIMARY KEY, task_id uuid NOT NULL, actor_subject text NOT NULL,
 idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
 command_hash text NOT NULL CHECK(command_hash ~ '^[0-9a-f]{64}$'),
 expected_revision bigint NOT NULL CHECK(expected_revision BETWEEN 1 AND 9007199254740989),
 assignment_id uuid NOT NULL UNIQUE REFERENCES sdlc_delivery_assignments(assignment_id) DEFERRABLE INITIALLY DEFERRED,
 execution_id uuid NOT NULL UNIQUE,execution_ordinal bigint NOT NULL CHECK(execution_ordinal>0),
 transaction_id xid8 NOT NULL DEFAULT pg_current_xact_id(),observations jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(task_id,actor_subject,idempotency_key),
 FOREIGN KEY(task_id,handoff_id) REFERENCES task_handoff_materials(task_id,handoff_id) ON DELETE RESTRICT
);
CREATE TABLE task_handoff_delivery_outcomes (
 handoff_id uuid PRIMARY KEY REFERENCES task_handoff_delivery_adoptions(handoff_id) ON DELETE RESTRICT,result jsonb NOT NULL
);
CREATE TABLE task_handoff_delivery_seed_permits (
 task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,transaction_id xid8 NOT NULL,
 handoff_id uuid NOT NULL REFERENCES task_handoff_delivery_adoptions(handoff_id) ON DELETE RESTRICT,
 previous_state jsonb NOT NULL,next_state jsonb NOT NULL,PRIMARY KEY(task_id,transaction_id)
);
CREATE TRIGGER task_handoff_adoption_immutable BEFORE UPDATE OR DELETE ON task_handoff_delivery_adoptions
 FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();
CREATE TRIGGER task_handoff_adoption_outcome_immutable BEFORE UPDATE OR DELETE ON task_handoff_delivery_outcomes
 FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE FUNCTION task_handoff_delivery_seed_allowed(previous sdlc_tasks,desired sdlc_tasks)
RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT previous.manual_control AND NOT desired.manual_control
 AND previous.pm_execution_id IS NOT DISTINCT FROM desired.pm_execution_id
 AND previous.pm_owner_version IS NOT DISTINCT FROM desired.pm_owner_version
 AND previous.pm_admission_state IS NOT DISTINCT FROM desired.pm_admission_state
 AND NOT task_lifecycle_runtime_busy(previous.task_id)
 AND EXISTS(SELECT 1 FROM task_handoff_delivery_seed_permits p
 JOIN task_handoff_delivery_adoptions a USING(task_id,handoff_id)
 JOIN task_handoff_materials m USING(task_id,handoff_id)
 JOIN sdlc_project_routing_heads h ON h.project_id=previous.project_id
 JOIN sdlc_source_policy_heads s ON s.project_id=previous.project_id
 WHERE p.task_id=previous.task_id AND p.transaction_id=pg_current_xact_id()
 AND a.transaction_id=pg_current_xact_id() AND p.previous_state=previous.state AND p.next_state=desired.state
 AND previous.state->>'human_handoff_id'=m.handoff_id::text AND m.execution_requested
 AND m.resulting_revision=previous.core_revision AND a.expected_revision=previous.core_revision
 AND m.actor_subject=previous.owner_subject
 AND m.receipt->'entry'->'state'->>'stage'=task_lifecycle_effective_stage(previous.state)
 AND m.receipt->'entry'->'state'->>'status'=task_lifecycle_effective_status(previous.state)
 AND task_lifecycle_effective_stage(previous.state) NOT IN ('Draft','Backlog')
 AND task_lifecycle_effective_status(previous.state) IN ('Ready','InProgress')
 AND NOT EXISTS(SELECT 1 FROM task_handoff_cancellations c WHERE c.handoff_id=m.handoff_id)
 AND desired.state->>'execution_handoff_id'=m.handoff_id::text AND NOT desired.state ? 'human_handoff_id'
 AND desired.state->>'interaction_protocol'='configured-cli-v1'
 AND desired.state->>'workflow_task_key'=COALESCE(previous.state->>'workflow_task_key','SDLC-'||a.execution_ordinal::text)
 AND desired.state->>'stage'=task_lifecycle_effective_stage(previous.state)
 AND desired.state->>'status'=task_lifecycle_effective_status(previous.state)
 AND desired.state->>'runtime_state'='queued' AND desired.state->>'waiting_reason'='queued_for_assignment'
 AND NOT desired.state ? 'draft_submode'
 AND ((desired.state->'lifecycle')-'resume_chat_id')=jsonb_build_object(
  'task_id',previous.task_id,'owner_subject',previous.owner_subject,'version',1,
  'stage',desired.state->>'stage','status',desired.state->>'status','cycle',0,'transient_retries',0,
  'waiting_reason','queued_for_assignment','runtime','queued','routing',m.routing,'previous_result_id',NULL)
 AND (NOT desired.state->'lifecycle' ? 'resume_chat_id' OR desired.state->'lifecycle'->'resume_chat_id'='null'::jsonb)
 AND (previous.state->>'workflow_task_key' IS NULL
  OR EXISTS(SELECT 1 FROM sdlc_pm_executions original WHERE original.task_id=previous.task_id
    AND original.result->'execution'->>'key'=previous.state->>'workflow_task_key')
  OR EXISTS(SELECT 1 FROM task_handoff_delivery_adoptions original JOIN task_handoff_delivery_outcomes o USING(handoff_id)
    WHERE original.task_id=previous.task_id AND 'SDLC-'||original.execution_ordinal::text=previous.state->>'workflow_task_key'))
 AND h.version=(m.routing->'policy'->>'version')::bigint AND s.source_revision=(m.source_binding->>'source_revision')::bigint
 AND previous.state-ARRAY['human_handoff_id','execution_handoff_id','interaction_protocol','workflow_task_key','lifecycle','runtime_state','waiting_reason','draft_submode','stage','status']
 =desired.state-ARRAY['human_handoff_id','execution_handoff_id','interaction_protocol','workflow_task_key','lifecycle','runtime_state','waiting_reason','draft_submode','stage','status'])
$$;
-- This classifies a genuine origin; it grants no write or transition by itself.
CREATE FUNCTION task_handoff_current_delivery_origin(task uuid,snapshot jsonb) RETURNS boolean
LANGUAGE sql STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM task_handoff_delivery_adoptions a JOIN task_handoff_materials m USING(task_id,handoff_id)
 WHERE a.task_id=task AND a.handoff_id::text=snapshot->>'execution_handoff_id'
 AND snapshot->>'interaction_protocol'='configured-cli-v1' AND (NOT snapshot ? 'lifecycle' OR snapshot->'lifecycle'->'routing'=m.routing)
 AND snapshot->>'owner_subject'=m.actor_subject AND snapshot->>'workflow_task_key' IS NOT NULL
 AND (a.transaction_id=pg_current_xact_id() OR EXISTS(SELECT 1 FROM task_handoff_delivery_outcomes o WHERE o.handoff_id=a.handoff_id)))
$$;
DO $$ DECLARE definition text; name text; BEGIN
 definition:=pg_get_functiondef('task_lifecycle_state_guard()'::regprocedure);
 IF position('    IF NEW.manual_control IS DISTINCT FROM OLD.manual_control' IN definition)=0 THEN RAISE EXCEPTION 'human_adoption_state_anchor_missing'; END IF;
 EXECUTE replace(definition,'    IF NEW.manual_control IS DISTINCT FROM OLD.manual_control',E'    IF task_handoff_delivery_seed_allowed(OLD,NEW) THEN NEW.core_revision:=OLD.core_revision+1; RETURN NEW; END IF;
    IF NEW.manual_control IS DISTINCT FROM OLD.manual_control');
 FOREACH name IN ARRAY ARRAY['sdlc_pm_reservation_gate','sdlc_delivery_canonical_gate','sdlc_configured_cli_completion_transition','sdlc_draft_flow_gate'] LOOP
 definition:=pg_get_functiondef(to_regprocedure(name||'()'));
 IF definition IS NULL OR position('BEGIN' IN definition)=0 THEN RAISE EXCEPTION 'human_adoption_guard_missing:%',name; END IF;
 EXECUTE overlay(definition placing E'BEGIN
    IF task_handoff_delivery_seed_allowed(OLD,NEW) THEN RETURN NEW; END IF;' from position('BEGIN' IN definition) for 5);
 END LOOP;
 definition:=pg_get_functiondef('sdlc_configured_cli_marker_gate()'::regprocedure);
 IF position('BEGIN' IN definition)=0 THEN RAISE EXCEPTION 'human_adoption_protocol_guard_missing'; END IF;
 definition:=overlay(definition placing E'BEGIN
    IF TG_OP=''UPDATE'' AND task_handoff_current_delivery_origin(NEW.task_id,NEW.state) THEN
        IF NOT task_handoff_delivery_seed_allowed(OLD,NEW) AND (OLD.state->''execution_handoff_id'' IS DISTINCT FROM NEW.state->''execution_handoff_id'' OR OLD.state->''interaction_protocol'' IS DISTINCT FROM NEW.state->''interaction_protocol'' OR OLD.state->''workflow_task_key'' IS DISTINCT FROM NEW.state->''workflow_task_key'') THEN RAISE EXCEPTION ''human execution origin is immutable'' USING ERRCODE=''23514''; END IF;
        RETURN NEW;
    END IF;' from position('BEGIN' IN definition) for 5);
 EXECUTE definition;
END $$;
DO $$ DECLARE definition text; anchor text:='head->''task''->>''status''=''Ready'''; BEGIN
 definition:=pg_get_functiondef('sdlc_delivery_assignment_gate()'::regprocedure);
 IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'human_assignment_status_anchor_missing'; END IF;
 EXECUTE replace(definition,anchor,'('||anchor||' OR (head->''task''->>''status''=''InProgress'' AND task_handoff_current_delivery_origin(NEW.task_id,task.state)))');
END $$;
CREATE FUNCTION task_handoff_delivery_adoption_committed() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE a sdlc_delivery_assignments;t sdlc_tasks;m task_handoff_materials;deadline timestamptz;
BEGIN
 SELECT * INTO STRICT a FROM sdlc_delivery_assignments WHERE assignment_id=NEW.assignment_id;
 SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=NEW.task_id;
 SELECT * INTO STRICT m FROM task_handoff_materials WHERE handoff_id=NEW.handoff_id AND task_id=NEW.task_id;
 deadline:=least((NEW.observations->>'fleet_observed_at')::timestamptz+interval '5 seconds',(NEW.observations->>'source_observed_at')::timestamptz+interval '30 seconds');
 IF NOT ((NEW.transaction_id=pg_current_xact_id()
 AND NEW.observations->>'issue_materials_sha256'=m.receipt->>'issue_materials_sha256'
 AND NEW.observations->'source_namespace'=m.source_binding->'namespace'
 AND NEW.observations->>'source_repository_id'=m.source_binding->>'repository_id'
 AND (NEW.observations->>'valid_until')::timestamptz=deadline AND deadline>clock_timestamp()
 AND t.core_revision=NEW.expected_revision+2 AND NOT t.manual_control
 AND task_handoff_current_delivery_origin(t.task_id,t.state)
 AND a.task_id=t.task_id AND a.execution_id=NEW.execution_id
 AND a.payload->'execution'->>'ordinal'=NEW.execution_ordinal::text
 AND a.payload->>'execution_id'=NEW.execution_id::text AND a.payload->>'owner_subject'=t.owner_subject AND a.payload->'routing'=m.routing
 AND EXISTS(SELECT 1 FROM sdlc_delivery_holds h WHERE h.assignment_id=a.assignment_id)
 AND EXISTS(SELECT 1 FROM task_handoff_delivery_outcomes o WHERE o.handoff_id=NEW.handoff_id
 AND o.result->'consent'=m.receipt AND o.result->'task'=t.state->'lifecycle'
 AND o.result->'assignment'->>'assignment_id'=a.assignment_id::text
 AND o.result->'assignment'->>'handoff_id'=m.handoff_id::text)) IS TRUE) THEN
 RAISE EXCEPTION 'task_handoff_native_adoption_invalid_or_expired' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER task_handoff_delivery_adoption_committed AFTER INSERT ON task_handoff_delivery_adoptions
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION task_handoff_delivery_adoption_committed();

-- Replacing a retained terminal head at a new human entry uses the same canonical
-- projection permit plus this exact seed; it does not forge a predecessor result.
CREATE FUNCTION task_handoff_delivery_seed_head_allowed(task uuid,cursor jsonb) RETURNS boolean
LANGUAGE sql STABLE AS $$
 SELECT cursor=jsonb_build_object('assignment',NULL,'lease',NULL,'binding',NULL,'result',NULL)
 AND EXISTS(SELECT 1 FROM task_handoff_delivery_seed_permits p
 JOIN task_handoff_delivery_adoptions a USING(task_id,handoff_id) JOIN sdlc_tasks t ON t.task_id=p.task_id
 WHERE p.task_id=task AND p.transaction_id=pg_current_xact_id() AND p.next_state=t.state
 AND t.core_revision=a.expected_revision+1 AND NOT t.manual_control
 AND task_handoff_current_delivery_origin(t.task_id,t.state))
$$;
DO $$ DECLARE definition text; anchor text:='IF NEW.execution->''assignment''=''null''::jsonb AND OLD.execution->''assignment''<>''null''::jsonb THEN'; BEGIN
 definition:=pg_get_functiondef('sdlc_delivery_head_gate()'::regprocedure);
 IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'human_seed_head_anchor_missing'; END IF;
 EXECUTE replace(definition,anchor,'IF NOT task_handoff_delivery_seed_head_allowed(NEW.task_id,NEW.execution) AND NEW.execution->''assignment''=''null''::jsonb AND OLD.execution->''assignment''<>''null''::jsonb THEN');
END $$;
-- Explicit current-origin scope selects the human source, never a latest legacy
-- binding. Retained legacy operations leave this transaction-local selector empty.
DO $$ DECLARE definition text; anchor text; replacement text; BEGIN
 definition:=pg_get_functiondef('tracker_bind_native_task(jsonb)'::regprocedure);
 anchor:=' SELECT payload->''namespace'' INTO entry FROM sdlc_task_source_bindings WHERE task_id=aggregate.task_id ORDER BY requirement_revision DESC LIMIT 1;';
 replacement:=$branch$
 IF NULLIF(current_setting('pdlc.tracker.current_handoff',true),'') IS NOT NULL THEN
  SELECT m.source_binding->'namespace' INTO entry FROM task_handoff_materials m JOIN sdlc_tasks t USING(task_id)
   WHERE m.task_id=aggregate.task_id AND m.handoff_id::text=current_setting('pdlc.tracker.current_handoff',true)
   AND ((t.manual_control AND t.state->>'human_handoff_id'=m.handoff_id::text AND m.execution_requested
    AND NOT EXISTS(SELECT 1 FROM task_handoff_cancellations c WHERE c.handoff_id=m.handoff_id))
    OR (NOT t.manual_control AND t.state->>'execution_handoff_id'=m.handoff_id::text
     AND task_handoff_current_delivery_origin(t.task_id,t.state)));
  IF NOT FOUND THEN RAISE EXCEPTION 'current_handoff_source_scope_changed' USING ERRCODE='42501'; END IF;
 ELSE
  SELECT payload->'namespace' INTO entry FROM sdlc_task_source_bindings WHERE task_id=aggregate.task_id ORDER BY requirement_revision DESC LIMIT 1;
 END IF;
$branch$;
 IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'human_native_source_anchor_missing'; END IF;
 definition:=replace(definition,anchor,replacement);
 anchor:='NOT EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=aggregate.task_id)';
 IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'human_native_drain_anchor_missing'; END IF;
 definition:=replace(definition,anchor,'NOT (EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=aggregate.task_id) OR (NULLIF(current_setting(''pdlc.tracker.current_handoff'',true),'''') IS NOT NULL AND EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=aggregate.task_id)))');
 EXECUTE definition;
END $$;

-- Preserve the required real execution key and accept an explicit human origin
-- alongside the historical PM creation origin. Do not write its old flow marker.
DO $$ DECLARE definition text; anchor text; BEGIN
 definition:=pg_get_functiondef('sdlc_delivery_draft_flow_gate()'::regprocedure);
 anchor:='AND t.state->''flow''=''"single-task-v1"''::jsonb';
 IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'human_assignment_origin_anchor_missing'; END IF;
 EXECUTE replace(definition,anchor,'AND (t.state->''flow''=''"single-task-v1"''::jsonb OR task_handoff_current_delivery_origin(t.task_id,t.state))');
END $$;

-- Technical projection may originate from genuine human consent instead of a PM
-- publication. Every existing canonical/result/fence guard and exact permit still runs.
DO $$ DECLARE definition text; BEGIN
 definition:=pg_get_functiondef('sdlc_configured_cli_completion_transition()'::regprocedure);
 IF position('BEGIN' IN definition)=0 THEN RAISE EXCEPTION 'human_technical_projection_anchor_missing'; END IF;
 EXECUTE overlay(definition placing E'BEGIN\n    IF TG_OP=''UPDATE'' AND task_handoff_current_delivery_origin(OLD.task_id,OLD.state) AND task_handoff_current_delivery_origin(NEW.task_id,NEW.state) AND EXISTS(SELECT 1 FROM sdlc_delivery_projection_permits p WHERE p.task_id=NEW.task_id AND p.transaction_id=pg_current_xact_id() AND p.next_state=NEW.state) THEN RETURN NEW; END IF;' from position('BEGIN' IN definition) for 5);
END $$;
