-- Later assignments retain the genuine human basis and real dependency ledger.
-- Historical operations keep NULL provenance; no old receipt is restamped.
-- The existing command/transaction_id pair belongs exclusively to historical
-- explicit Analysis start. Do not reinterpret or broaden that strict contract.
ALTER TABLE sdlc_delivery_operations ADD COLUMN handoff_assignment_transaction xid8;
ALTER TABLE sdlc_delivery_operations ADD CONSTRAINT handoff_assignment_operation CHECK (
 handoff_assignment_transaction IS NULL OR (command IS NULL AND transaction_id IS NULL
  AND event_id IS NOT NULL AND result->'assignment' IS NOT NULL AND result->'assignment'<>'null'::jsonb)
);

CREATE OR REPLACE FUNCTION tracker_current_stage_grant_committed() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE a task_handoff_delivery_adoptions;m task_handoff_materials;assignment sdlc_delivery_assignments;
 t sdlc_tasks;caller jsonb;source jsonb;entry jsonb;initial boolean;
BEGIN
 IF NEW.origin<>'delivery' THEN RETURN NEW;END IF;
 caller:=tracker_current_native_caller();
 SELECT * INTO STRICT t FROM sdlc_tasks WHERE task_id=NEW.task_id;
 SELECT * INTO STRICT a FROM task_handoff_delivery_adoptions WHERE task_id=NEW.task_id AND handoff_id::text=t.state->>'execution_handoff_id';
 SELECT * INTO STRICT m FROM task_handoff_materials WHERE handoff_id=a.handoff_id AND task_id=a.task_id;
 SELECT * INTO STRICT assignment FROM sdlc_delivery_assignments WHERE assignment_id=NEW.assignment_id AND task_id=NEW.task_id;
 initial:=a.assignment_id=NEW.assignment_id;
 source:=jsonb_build_object('registry_instance_id',m.source_binding->'namespace'->>'registry_instance_id','space_id',m.source_binding->'namespace'->>'namespace_id');
 IF NOT task_handoff_current_delivery_origin(t.task_id,t.state)
  OR NEW.project_id<>(m.receipt->>'project_id')::uuid OR NEW.assignment_version IS NOT NULL
  OR caller->'writable' IS DISTINCT FROM 'true'::jsonb OR caller->'identity'->>'subject' IS DISTINCT FROM a.actor_subject
  OR NEW.original_actor IS DISTINCT FROM jsonb_build_object('issuer',caller->'identity'->>'issuer','subject',m.actor_subject)
  OR NEW.initial_valid_until<=floor(extract(epoch FROM clock_timestamp()))::bigint
  OR NEW.initial_valid_until>(caller->'validity'->>'valid_until')::bigint
  OR NEW.required_wiki_space IS NOT NULL OR NOT NEW.spaces @> jsonb_build_array(source)
  OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(NEW.spaces))<>jsonb_array_length(NEW.spaces)
  OR assignment.payload->'routing' IS DISTINCT FROM m.routing
  OR NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds h WHERE h.assignment_id=NEW.assignment_id AND h.task_id=NEW.task_id)
  OR (initial AND (a.transaction_id<>pg_current_xact_id()
   OR NEW.initial_valid_until>floor(extract(epoch FROM (a.observations->>'valid_until')::timestamptz))::bigint))
  OR (NOT initial AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_operations o
   WHERE o.task_id=NEW.task_id AND o.actor_subject=caller->'identity'->>'subject'
    AND o.handoff_assignment_transaction=pg_current_xact_id() AND o.result->'assignment'=assignment.payload
    AND o.result->'task'=t.state->'lifecycle' AND o.result->'lease'->>'lease_id'=assignment.payload->>'lease_id'))
 THEN RAISE EXCEPTION 'current_stage_execution_dependencies_invalid' USING ERRCODE='23514';END IF;
 FOR entry IN SELECT value FROM jsonb_array_elements(NEW.spaces) LOOP
  IF NOT tracker_valid_space_ref(entry)
   OR (entry<>source AND NOT EXISTS(SELECT 1 FROM tracker_project_spaces l WHERE l.project_id=NEW.project_id AND l.registry_instance_id::text=entry->>'registry_instance_id' AND l.space_id::text=entry->>'space_id'))
   OR NOT EXISTS(SELECT 1 FROM tracker_space_catalog c WHERE c.registry_instance_id::text=entry->>'registry_instance_id' AND c.space_id::text=entry->>'space_id' AND c.admin_revision IS NOT NULL AND c.state='active')
  THEN RAISE EXCEPTION 'current_stage_execution_dependencies_invalid' USING ERRCODE='23514';END IF;
 END LOOP;
 RETURN NEW;
END $$;
CREATE FUNCTION task_handoff_assignment_dependencies_committed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=NEW.task_id AND task_handoff_current_delivery_origin(t.task_id,t.state))
  AND (NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL
   OR (SELECT active FROM tracker_space_admission_state WHERE singleton)
   OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL))
  AND NOT EXISTS(SELECT 1 FROM tracker_execution_space_grants g WHERE g.assignment_id=NEW.assignment_id AND g.task_id=NEW.task_id AND g.origin='delivery')
 THEN RAISE EXCEPTION 'current_stage_assignment_dependencies_required' USING ERRCODE='23514';END IF;
 RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER task_handoff_assignment_dependencies_committed AFTER INSERT ON sdlc_delivery_assignments
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION task_handoff_assignment_dependencies_committed();

DO $$ DECLARE definition text;anchor text;BEGIN
 definition:=pg_get_functiondef('tracker_bind_native_task(jsonb)'::regprocedure);
 anchor:=$old$FROM tracker_execution_space_grants g
   JOIN task_handoff_delivery_adoptions a USING(assignment_id,task_id)
   JOIN sdlc_delivery_holds h USING(assignment_id,task_id)$old$;
 IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'current_assignment_scope_anchor_missing';END IF;
 EXECUTE replace(definition,anchor,$new$FROM tracker_execution_space_grants g
   JOIN task_handoff_delivery_adoptions a ON a.task_id=g.task_id
   JOIN sdlc_delivery_holds h ON h.assignment_id=g.assignment_id AND h.task_id=g.task_id$new$);
END $$;
