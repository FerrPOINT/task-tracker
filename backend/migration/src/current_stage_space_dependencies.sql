-- Add a genuine technical origin without rewriting PM/Analysis receipts.
ALTER TABLE tracker_execution_space_grants DROP CONSTRAINT tracker_execution_space_grants_origin_check;
ALTER TABLE tracker_execution_space_grants ADD CONSTRAINT tracker_execution_space_grants_origin_check CHECK(origin IN ('pm','analysis','delivery'));
ALTER TABLE tracker_execution_space_grants DROP CONSTRAINT tracker_execution_space_grants_check;
ALTER TABLE tracker_execution_space_grants ADD CONSTRAINT tracker_execution_space_grants_check CHECK((origin='pm' AND assignment_version IS NOT NULL AND assignment_version>0) OR (origin IN ('analysis','delivery') AND assignment_version IS NULL));
-- A project has <=1000 spaces; its explicit repository source may be outside them.
ALTER TABLE tracker_execution_space_grants DROP CONSTRAINT tracker_execution_space_grants_spaces_check;
ALTER TABLE tracker_execution_space_grants ADD CONSTRAINT tracker_execution_space_grants_spaces_check CHECK(jsonb_typeof(spaces)='array' AND jsonb_array_length(spaces) BETWEEN 1 AND CASE WHEN origin='delivery' THEN 1001 ELSE 1000 END);
CREATE FUNCTION tracker_current_stage_grant_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN
  IF OLD.origin='delivery' THEN RAISE EXCEPTION 'technical execution dependencies are immutable' USING ERRCODE='23514'; END IF;RETURN OLD;
 END IF;
 IF (OLD.origin='delivery' OR NEW.origin='delivery') AND NEW IS DISTINCT FROM OLD THEN RAISE EXCEPTION 'technical execution dependencies are immutable' USING ERRCODE='23514';END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER tracker_current_stage_grant_immutable BEFORE UPDATE OR DELETE ON tracker_execution_space_grants FOR EACH ROW EXECUTE FUNCTION tracker_current_stage_grant_immutable();
CREATE FUNCTION tracker_current_stage_grant_committed() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE a task_handoff_delivery_adoptions;m task_handoff_materials;assignment sdlc_delivery_assignments;caller jsonb;source jsonb;entry jsonb;
BEGIN
 IF NEW.origin<>'delivery' THEN RETURN NEW;END IF;
 caller:=tracker_current_native_caller();
 SELECT * INTO STRICT a FROM task_handoff_delivery_adoptions WHERE assignment_id=NEW.assignment_id AND task_id=NEW.task_id;
 SELECT * INTO STRICT m FROM task_handoff_materials WHERE handoff_id=a.handoff_id AND task_id=a.task_id;
 SELECT * INTO STRICT assignment FROM sdlc_delivery_assignments WHERE assignment_id=NEW.assignment_id AND task_id=NEW.task_id;
 source:=jsonb_build_object('registry_instance_id',m.source_binding->'namespace'->>'registry_instance_id','space_id',m.source_binding->'namespace'->>'namespace_id');
 IF a.transaction_id<>pg_current_xact_id() OR NEW.project_id<>(m.receipt->>'project_id')::uuid OR NEW.assignment_version IS NOT NULL
  OR caller->'writable' IS DISTINCT FROM 'true'::jsonb OR caller->'identity'->>'subject' IS DISTINCT FROM a.actor_subject
  OR NEW.original_actor IS DISTINCT FROM jsonb_build_object('issuer',caller->'identity'->>'issuer','subject',m.actor_subject)
  OR NEW.initial_valid_until<=floor(extract(epoch FROM clock_timestamp()))::bigint
  OR NEW.initial_valid_until>(caller->'validity'->>'valid_until')::bigint
  OR NEW.initial_valid_until>floor(extract(epoch FROM (a.observations->>'valid_until')::timestamptz))::bigint
  OR NEW.required_wiki_space IS NOT NULL OR NOT NEW.spaces @> jsonb_build_array(source)
  OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(NEW.spaces))<>jsonb_array_length(NEW.spaces)
  OR NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds h WHERE h.assignment_id=NEW.assignment_id AND h.task_id=NEW.task_id)
 THEN RAISE EXCEPTION 'current_stage_execution_dependencies_invalid' USING ERRCODE='23514';END IF;
 FOR entry IN SELECT value FROM jsonb_array_elements(NEW.spaces) LOOP
  IF NOT tracker_valid_space_ref(entry) OR (entry<>source AND NOT EXISTS(SELECT 1 FROM tracker_project_spaces l WHERE l.project_id=NEW.project_id AND l.registry_instance_id::text=entry->>'registry_instance_id' AND l.space_id::text=entry->>'space_id')) THEN RAISE EXCEPTION 'current_stage_execution_dependencies_invalid' USING ERRCODE='23514';END IF;
 END LOOP;
 RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER tracker_current_stage_grant_committed AFTER INSERT ON tracker_execution_space_grants DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_current_stage_grant_committed();
CREATE FUNCTION task_handoff_dependencies_committed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF (NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL OR (SELECT active FROM tracker_space_admission_state WHERE singleton) OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL))
 AND NOT EXISTS(SELECT 1 FROM tracker_execution_space_grants g WHERE g.assignment_id=NEW.assignment_id AND g.task_id=NEW.task_id AND g.origin='delivery') THEN RAISE EXCEPTION 'current_stage_execution_dependencies_required' USING ERRCODE='23514';END IF;
 RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER task_handoff_dependencies_committed AFTER INSERT ON task_handoff_delivery_adoptions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION task_handoff_dependencies_committed();
