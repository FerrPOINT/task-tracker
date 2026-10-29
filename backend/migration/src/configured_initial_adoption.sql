-- Owner reservation may adopt an untouched pre-protocol Draft. No migration
-- rewrites tasks, original input, creation receipts or published history.
CREATE FUNCTION sdlc_configured_initial_adoption(previous sdlc_tasks, next sdlc_tasks)
RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT (
        NOT previous.state ? 'interaction_protocol'
        AND previous.task_id=previous.root_task_id
        AND previous.pm_owner_version=0 AND previous.pm_execution_id IS NULL
        AND previous.pm_admission_state IS NULL
        AND previous.state->>'stage'='Draft'
        AND COALESCE(previous.state->>'status','Ready')='Ready'
        AND COALESCE(previous.state->>'draft_submode','brainstorming')='brainstorming'
        AND COALESCE(previous.state->>'flow','single-task-v1')='single-task-v1'
        AND COALESCE(previous.state->'assignment','null'::jsonb)='null'::jsonb
        AND COALESCE(previous.state->'confirmation_revision','null'::jsonb)='null'::jsonb
        AND COALESCE(previous.state->'questions','[]'::jsonb)='[]'::jsonb
        AND COALESCE(previous.state->'revisions','[]'::jsonb)='[]'::jsonb
        AND COALESCE(previous.state->'confirmations','[]'::jsonb)='[]'::jsonb
        AND COALESCE(previous.state->'evidence','[]'::jsonb)='[]'::jsonb
        AND NOT previous.state ?| ARRAY['lifecycle','workflow_task_key']
        AND next.state->>'interaction_protocol'='configured-cli-v1'
        AND next.state->>'flow'='single-task-v1'
        AND next.state->>'status'='Ready'
        AND next.state->>'draft_submode'='brainstorming'
        AND previous.state-ARRAY['interaction_protocol','flow','status','draft_submode']
            =next.state-ARRAY['interaction_protocol','flow','status','draft_submode']
        AND to_jsonb(previous)-'state'=to_jsonb(next)-'state'
        AND EXISTS(SELECT 1 FROM sdlc_draft_creations c WHERE c.task_id=previous.task_id
            AND c.project_id=previous.project_id AND c.actor_subject=previous.owner_subject
            AND c.input_snapshot_ref IS NOT NULL)
        AND NOT EXISTS(SELECT 1 FROM sdlc_assignments a WHERE a.task_id=previous.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_pm_executions e WHERE e.task_id=previous.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_confirmations c WHERE c.task_id=previous.task_id)
        AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_heads h WHERE h.task_id=previous.task_id)
    ) IS TRUE
$$;

-- Extend the two existing guards, retaining every other rule and its order.
DO $$
DECLARE name text; anchor text; definition text;
BEGIN
    FOR name,anchor IN SELECT * FROM (VALUES
        ('sdlc_configured_cli_marker_gate',
         'TG_OP=''UPDATE'' AND OLD.state->''interaction_protocol'' IS DISTINCT FROM NEW.state->''interaction_protocol'''),
        ('sdlc_draft_flow_gate',
         'OLD.state->''flow'' IS DISTINCT FROM NEW.state->''flow''')
    ) AS guards(name,anchor)
    LOOP
        definition:=pg_get_functiondef((name||'()')::regprocedure);
        IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
            RAISE EXCEPTION 'initial configured adoption guard anchor changed: %',name;
        END IF;
        EXECUTE replace(definition,anchor,'('||anchor||') AND NOT sdlc_configured_initial_adoption(OLD,NEW)');
    END LOOP;
END $$;
