-- Configured CLI questions omit native checkpoint_id. SQL NULL (missing key)
-- and explicit JSON null both mean absence; a concrete checkpoint stays invalid.
-- Patch the released function without changing any other lifecycle guard.
DO $patch$
DECLARE
    definition text := pg_get_functiondef('sdlc_critical_pm_lifecycle_insert_gate()'::regprocedure);
    old_body text := $anchor$body->'checkpoint_id' IS DISTINCT FROM 'null'::jsonb$anchor$;
    old_result text := $anchor$NEW.result->'checkpoint_id' IS DISTINCT FROM 'null'::jsonb$anchor$;
BEGIN
    IF (length(definition)-length(replace(definition,old_body,''))) / length(old_body) <> 1
        OR (length(definition)-length(replace(definition,old_result,''))) / length(old_result) <> 1 THEN
        RAISE EXCEPTION 'unexpected configured checkpoint guard definition';
    END IF;
    definition := replace(definition,old_body,
        $anchor$COALESCE(body->'checkpoint_id','null'::jsonb) IS DISTINCT FROM 'null'::jsonb$anchor$);
    definition := replace(definition,old_result,
        $anchor$COALESCE(NEW.result->'checkpoint_id','null'::jsonb) IS DISTINCT FROM 'null'::jsonb$anchor$);
    EXECUTE definition;
END;
$patch$;
