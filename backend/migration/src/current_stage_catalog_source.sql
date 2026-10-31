-- Source ownership follows the exact selected repository for new human consent.
-- Historical PM/task-source receipts and strict envelopes are not rewritten.
CREATE FUNCTION task_handoff_source_policy_current(project uuid, binding jsonb) RETURNS boolean
LANGUAGE sql STABLE AS $$
 SELECT EXISTS(
  SELECT 1 FROM sdlc_source_policy_revisions s JOIN sdlc_source_policy_heads h USING(project_id)
  WHERE s.project_id=$1 AND s.source_revision=h.source_revision
   AND binding->'source_revision'=to_jsonb(s.source_revision)
   AND binding->>'policy_hash'=s.source_hash
   AND binding->>'forge_instance_id'=s.payload->'source'->>'forge_instance_id'
   AND binding->>'repository_id'=s.payload->'source'->>'repository_id'
   AND binding->>'base_ref'=s.payload->'source'->>'base_ref'
   AND binding->>'base_commit'=s.payload->'source'->>'base_commit'
   AND binding->'namespace'=s.payload->'verification'->'namespace'
   AND tracker_valid_space_ref(jsonb_build_object('registry_instance_id',binding->'namespace'->'registry_instance_id','space_id',binding->'namespace'->'namespace_id'))
   AND (
    EXISTS(SELECT 1 FROM project_repository_links l WHERE l.project_id=s.project_id
     AND l.forge_instance_id::text=s.payload->'source'->>'forge_instance_id'
     AND l.repository_id::text=s.payload->'source'->>'repository_id'
     AND l.snapshot->>'forge_instance_id'=l.forge_instance_id::text
     AND l.snapshot->>'repository_id'=l.repository_id::text
     AND l.snapshot->>'kind'='hosted' AND l.snapshot->'namespace'=binding->'namespace')
    OR (NOT EXISTS(SELECT 1 FROM project_repository_links l WHERE l.project_id=s.project_id
      AND l.forge_instance_id::text=s.payload->'source'->>'forge_instance_id'
      AND l.repository_id::text=s.payload->'source'->>'repository_id')
     AND EXISTS(SELECT 1 FROM tracker_namespace_bindings b WHERE b.resource_id=s.project_id AND b.state='active'
      AND binding->'namespace'->>'registry_instance_id'=b.registry_instance_id::text
      AND binding->'namespace'->>'namespace_id'=b.namespace_id::text))
   )
 )
$$;
DO $$ DECLARE definition text; anchor text; BEGIN
 definition:=pg_get_functiondef('task_handoff_material_write_permitted(uuid,jsonb)'::regprocedure);
 anchor:=$old$                AND EXISTS(SELECT 1 FROM sdlc_source_policy_revisions s
                    JOIN sdlc_source_policy_heads h USING(project_id)
                    JOIN tracker_namespace_bindings b ON b.resource_id=s.project_id AND b.state='active'
                    WHERE s.project_id=t.project_id AND s.source_revision=h.source_revision
                        AND m.source_binding->'source_revision'=to_jsonb(s.source_revision)
                        AND m.source_binding->>'policy_hash'=s.source_hash
                        AND m.source_binding->'namespace'=s.payload->'verification'->'namespace'
                        AND m.source_binding->'namespace'->>'registry_instance_id'=b.registry_instance_id::text
                        AND m.source_binding->'namespace'->>'namespace_id'=b.namespace_id::text)$old$;
 IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'human_source_policy_guard_anchor_missing';END IF;
 definition:=replace(definition,anchor,'                AND task_handoff_source_policy_current(t.project_id,m.source_binding)');
 EXECUTE definition;
END $$;


-- Current human execution gets resource scope from its genuine source/dependency
-- receipts. Legacy executions retain their explicit historical 1:1 owner check.
DO $$ DECLARE definition text; anchor text; BEGIN
 definition:=pg_get_functiondef('tracker_bind_native_task(jsonb)'::regprocedure);
 anchor:=$old$ SELECT registry_instance_id,namespace_id INTO owner FROM tracker_namespace_bindings WHERE resource_id=aggregate.project_id FOR SHARE;
 IF NOT FOUND THEN RAISE EXCEPTION 'native_namespace_binding_required' USING ERRCODE='42501';END IF;
 spaces:=jsonb_build_array(jsonb_build_object('registry_instance_id',owner.registry_instance_id,'space_id',owner.namespace_id));$old$;
 IF position(anchor IN definition)=0 THEN RAISE EXCEPTION 'human_native_project_owner_anchor_missing';END IF;
 definition:=replace(definition,anchor,$new$
 IF NULLIF(current_setting('pdlc.tracker.current_handoff',true),'') IS NOT NULL THEN
  SELECT g.spaces INTO spaces FROM tracker_execution_space_grants g
   JOIN task_handoff_delivery_adoptions a USING(assignment_id,task_id)
   JOIN sdlc_delivery_holds h USING(assignment_id,task_id)
   WHERE g.task_id=aggregate.task_id AND g.project_id=aggregate.project_id AND g.origin='delivery'
    AND a.handoff_id::text=current_setting('pdlc.tracker.current_handoff',true) FOR SHARE OF g,a,h;
  spaces:=COALESCE(spaces,'[]'::jsonb);
 ELSE
  SELECT registry_instance_id,namespace_id INTO owner FROM tracker_namespace_bindings WHERE resource_id=aggregate.project_id FOR SHARE;
  IF NOT FOUND THEN RAISE EXCEPTION 'native_namespace_binding_required' USING ERRCODE='42501';END IF;
  spaces:=jsonb_build_array(jsonb_build_object('registry_instance_id',owner.registry_instance_id,'space_id',owner.namespace_id));
 END IF;
$new$);
 -- The existing exact-current-handoff source branch below still validates its
 -- immutable source receipt and adds the source. It cannot leave an empty scope.
 EXECUTE definition;
END $$;
