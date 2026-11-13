-- Only the mutable current projection loses ownership clocks. No stored row,
-- immutable operation result, assignment hash or PM execution lease is rewritten.
CREATE OR REPLACE VIEW sdlc_delivery_tasks AS SELECT t.task_id,
    jsonb_build_object('task',t.state->'lifecycle') ||
    CASE WHEN jsonb_typeof(h.execution->'lease')='object'
        THEN jsonb_set(h.execution,'{lease}',(h.execution->'lease')-ARRAY['heartbeat_at','expires_at'])
        ELSE h.execution END AS receipt
    FROM sdlc_tasks t JOIN sdlc_delivery_heads h USING(task_id);

DO $patch$
DECLARE name text; source text; old text;
BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_delivery_run_gate()', 'sdlc_delivery_proposal_gate()',
        'sdlc_delivery_result_gate()', 'sdlc_agent_status_projection(uuid,jsonb,jsonb)'] LOOP
        source:=replace(pg_get_functiondef(to_regprocedure(name)),E'\r\n',E'\n');
        old:=$anchor$AND (head->'lease'->>'expires_at')::timestamptz>clock_timestamp()$anchor$;
        IF source IS NULL OR (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
            RAISE EXCEPTION 'untimed delivery guard anchor changed: %',name;
        END IF;
        EXECUTE replace(source,old,'');
    END LOOP;

    source:=replace(pg_get_functiondef('sdlc_critical_blocker_insert_gate()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$AND (head->'lease'->>'expires_at')::timestamptz>expected_time
            AND (NEW.payload->>'created_at')::timestamptz<=(head->'lease'->>'expires_at')::timestamptz$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'untimed critical blocker clock anchor changed';
    END IF;
    source:=replace(source,old,'');
    old:=$anchor$(body->'fence'->>'expected_lease_version')::bigint
                BETWEEN 1 AND (head->'lease'->>'version')::bigint$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'untimed critical blocker version anchor changed';
    END IF;
    EXECUTE replace(source,old,$anchor$(body->'fence'->>'expected_lease_version')::bigint
                = (head->'lease'->>'version')::bigint$anchor$);

    source:=replace(pg_get_functiondef('sdlc_delivery_head_gate()'::regprocedure),E'\r\n',E'\n');
    old:=$anchor$IF ((next_lease->>'version')::bigint=(previous_lease->>'version')::bigint+1
                AND (previous_lease->>'expires_at')::timestamptz>clock_timestamp()
                AND next_lease->'lease_id'=previous_lease->'lease_id'
                AND next_lease->'holder_subject'=previous_lease->'holder_subject'
                AND next_lease->'claimed_at'=previous_lease->'claimed_at'
                AND (next_lease->>'expires_at')::timestamptz=
                    (next_lease->>'heartbeat_at')::timestamptz+interval '30 seconds') IS NOT TRUE$anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'untimed delivery binding immutability anchor changed';
    END IF;
    source:=replace(source,old,$anchor$IF (next_lease=previous_lease-ARRAY['heartbeat_at','expires_at']) IS NOT TRUE$anchor$);
    old:=$anchor$    RETURN NEW;
END $anchor$;
    IF (length(source)-length(replace(source,old,'')))/length(old)<>1 THEN
        RAISE EXCEPTION 'untimed delivery binding shape anchor changed';
    END IF;
    EXECUTE replace(source,old,$anchor$    next_lease:=NEW.execution->'lease';
    IF next_lease IS DISTINCT FROM 'null'::jsonb AND (
        jsonb_typeof(next_lease)='object'
        AND next_lease ?& ARRAY['lease_id','version','holder_subject','claimed_at']
        AND next_lease-ARRAY['lease_id','version','holder_subject','claimed_at']='{}'::jsonb
        AND jsonb_typeof(next_lease->'version')='number'
        AND (next_lease->>'version')::bigint BETWEEN 1 AND 9007199254740991
        AND jsonb_typeof(next_lease->'holder_subject')='string'
        AND length(next_lease->>'holder_subject')>0
        AND next_lease->'lease_id'=NEW.execution->'assignment'->'lease_id') IS NOT TRUE THEN
        RAISE EXCEPTION 'SDLC exact untimed delivery binding required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $anchor$);
END $patch$;
