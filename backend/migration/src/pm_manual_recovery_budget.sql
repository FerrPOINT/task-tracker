-- Forward 0104: retain the existing settled-input transaction and all old guards.
-- The optional permission comes only from the authenticated Fleet callback;
-- its prefix hash seals Fleet commands, not a fabricated Tracker projection.
LOCK TABLE sdlc_tasks,sdlc_pm_execution_leases,sdlc_pm_resume_inputs,sdlc_pm_lease_operations
    IN ACCESS EXCLUSIVE MODE;

CREATE FUNCTION sdlc_pm_settled_manual_budget_allows(witness jsonb, command jsonb, attempts bigint)
RETURNS boolean LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE budget jsonb; prefix numeric;
BEGIN
    IF attempts IS NULL OR attempts<0 THEN RETURN false; END IF;
    IF NOT (witness ? 'manual_budget') THEN RETURN attempts<3; END IF;
    budget:=witness->'manual_budget';
    IF (jsonb_typeof(budget)='object'
        AND budget ?& ARRAY['contract_version','root_operation_id','control_operation_id',
            'retry_operation_key','retry_request_sha256','original_resume_request_sha256',
            'prefix_count','prefix_sha256','allowance']
        AND budget-ARRAY['contract_version','root_operation_id','control_operation_id',
            'retry_operation_key','retry_request_sha256','original_resume_request_sha256',
            'prefix_count','prefix_sha256','allowance']='{}'::jsonb
        AND jsonb_typeof(budget->'contract_version')='number' AND budget->>'contract_version'='1'
        AND jsonb_typeof(budget->'allowance')='number' AND budget->>'allowance'='3'
        AND jsonb_typeof(budget->'prefix_count')='number'
        AND budget->>'prefix_count' ~ '^[1-9][0-9]*$'
        AND length(budget->>'prefix_count')<=16
        AND NOT EXISTS(SELECT 1 FROM unnest(ARRAY['root_operation_id','control_operation_id',
            'retry_operation_key','retry_request_sha256','original_resume_request_sha256',
            'prefix_sha256']) AS fields(name) WHERE jsonb_typeof(budget->name) IS DISTINCT FROM 'string')
        AND budget->>'root_operation_id' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
        AND budget->>'control_operation_id' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
        AND budget->>'root_operation_id'<>'00000000-0000-0000-0000-000000000000'
        AND budget->>'control_operation_id'<>'00000000-0000-0000-0000-000000000000'
        AND budget->>'retry_operation_key' ~ '^fleet-pm-resume-retry:[0-9a-f]{64}$'
        AND budget->>'retry_request_sha256' ~ '^[0-9a-f]{64}$'
        AND budget->>'original_resume_request_sha256' ~ '^[0-9a-f]{64}$'
        AND budget->>'prefix_sha256' ~ '^[0-9a-f]{64}$'
        AND budget->>'original_resume_request_sha256'=command#>>'{recovery,original_request_sha256}') IS NOT TRUE
    THEN RETURN false; END IF;
    prefix:=(budget->>'prefix_count')::numeric;
    RETURN prefix>=3 AND prefix+3<=9007199254740991
        AND attempts>=prefix AND attempts<prefix+3;
END $$;

-- Preserve the old function OID/body except the cap and optional witness member.
DO $manual_budget_gate$
DECLARE definition text; anchor text; replacement text;
BEGIN
    SELECT pg_get_functiondef('sdlc_pm_settled_input_recovery_gate()'::regprocedure)
        INTO STRICT definition;
    anchor:='AND NEW.idempotency_key<>predecessor_key AND attempts<3';
    replacement:='AND NEW.idempotency_key<>predecessor_key AND sdlc_pm_settled_manual_budget_allows(w,c,attempts)';
    IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'Settled PM recovery cap source differs'; END IF;
    definition:=replace(definition,anchor,replacement);
    anchor:=$old$AND w-ARRAY['contract_version','session_run_id','barrier_id','identity','request_sha256','command']='{}'::jsonb$old$;
    replacement:=$new$AND w-ARRAY['contract_version','session_run_id','barrier_id','identity','request_sha256','command','manual_budget']='{}'::jsonb$new$;
    IF (length(definition)-length(replace(definition,anchor,'')))/length(anchor)<>1 THEN
        RAISE EXCEPTION 'Settled PM recovery witness source differs'; END IF;
    EXECUTE replace(definition,anchor,replacement);
END $manual_budget_gate$;
