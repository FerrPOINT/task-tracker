-- Change only the Reviewer destination. Keep the existing producer, proposal,
-- terminal receipt, assignment, lease and canonical projection guards intact.
DO $patch$
DECLARE source text; old text;
BEGIN
    source:=replace(pg_get_functiondef('sdlc_delivery_result_gate()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$    expected_status := CASE WHEN role='devops' THEN 'Completed' ELSE 'Ready' END;
    expected_stage := CASE role WHEN 'analyst' THEN 'Architecture' WHEN 'architect' THEN 'Development'
        WHEN 'developer' THEN 'Review' WHEN 'reviewer' THEN 'Testing'
        WHEN 'tester' THEN 'Deployment' WHEN 'devops' THEN 'Deployment' END;
    IF role IN ('reviewer','tester') THEN
        IF verdict='rework' THEN
            IF expected_cycle=3 THEN expected_stage := head->'task'->>'stage'; expected_status := 'Completed';
            ELSE expected_stage := 'Rework'; expected_cycle := expected_cycle+1; END IF;
        ELSIF (role='reviewer' AND verdict='approved') OR (role='tester' AND verdict='passed') THEN
            NULL;
        ELSE RAISE EXCEPTION 'SDLC invalid typed review/testing verdict' USING ERRCODE='23514'; END IF;
    END IF;
$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'Reviewer result transition anchor changed';
    END IF;
    EXECUTE replace(source,old,$anchor$    expected_status := CASE WHEN role IN ('reviewer','devops') THEN 'Completed' ELSE 'Ready' END;
    expected_stage := CASE role WHEN 'analyst' THEN 'Architecture' WHEN 'architect' THEN 'Development'
        WHEN 'developer' THEN 'Review' WHEN 'reviewer' THEN 'Review'
        WHEN 'tester' THEN 'Deployment' WHEN 'devops' THEN 'Deployment' END;
    IF role='reviewer' THEN
        IF (verdict IN ('approved','rework')) IS NOT TRUE THEN
            RAISE EXCEPTION 'SDLC invalid typed review verdict' USING ERRCODE='23514';
        END IF;
    ELSIF role='tester' THEN
        IF verdict='rework' THEN
            IF expected_cycle=3 THEN expected_stage := head->'task'->>'stage'; expected_status := 'Completed';
            ELSE expected_stage := 'Rework'; expected_cycle := expected_cycle+1; END IF;
        ELSIF verdict='passed' THEN NULL;
        ELSE RAISE EXCEPTION 'SDLC invalid typed testing verdict' USING ERRCODE='23514'; END IF;
    END IF;
$anchor$);
END $patch$;
