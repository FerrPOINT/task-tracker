-- These invariant cases have a valid original create proof, so a missing
-- request context cannot mask failure of the independent nonempty constraint.
CREATE FUNCTION qa_bind_creation(project uuid) RETURNS void LANGUAGE plpgsql AS $$ BEGIN
 PERFORM set_config('pdlc.tracker.write_actor',jsonb_build_object('schema_version',1,'kind','create_project','project_id',project,'user_id','00000000-0000-4000-8000-000000000030','identity',jsonb_build_object('issuer','https://auth.example.test','subject','owner-subject'),'active_spaces',jsonb_build_array(jsonb_build_object('registry_instance_id','00000000-0000-4000-8000-000000000010','space_id','00000000-0000-4000-8000-000000000011')),'readable_spaces',jsonb_build_array(jsonb_build_object('registry_instance_id','00000000-0000-4000-8000-000000000010','space_id','00000000-0000-4000-8000-000000000011')),'requested_spaces',jsonb_build_array(jsonb_build_object('registry_instance_id','00000000-0000-4000-8000-000000000010','space_id','00000000-0000-4000-8000-000000000011')),'policy_revision',1,'writable',true,'valid_until',floor(extract(epoch FROM clock_timestamp()))::bigint+60)::text,true) ;
END $$;
DO $$ BEGIN
    IF (SELECT count(*) FROM tracker_project_spaces) <> 2 THEN RAISE EXCEPTION 'backfill changed identities'; END IF;
    IF (SELECT count(*) FROM tracker_space_catalog WHERE admin_revision IS NOT NULL) <> 0 THEN RAISE EXCEPTION 'legacy generation became catalog revision'; END IF;
    BEGIN
        UPDATE tracker_space_admission_state SET active=true;
        RAISE EXCEPTION 'unconfirmed activation accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
END $$;
UPDATE tracker_space_catalog SET admin_revision=10;
UPDATE tracker_space_admission_state SET active=true;
DO $$ BEGIN
    BEGIN
        PERFORM qa_bind_creation('00000000-0000-4000-8000-000000000003');
        INSERT INTO projects(id) VALUES ('00000000-0000-4000-8000-000000000003');
        SET CONSTRAINTS ALL IMMEDIATE;
        RAISE EXCEPTION 'unlinked project accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
END $$;
BEGIN;
SELECT qa_bind_creation('00000000-0000-4000-8000-000000000003');
        INSERT INTO projects(id) VALUES ('00000000-0000-4000-8000-000000000003');
INSERT INTO tracker_project_spaces VALUES ('00000000-0000-4000-8000-000000000003','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000011',now());
COMMIT;
BEGIN;
INSERT INTO tracker_project_spaces VALUES ('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000012',now());
DELETE FROM tracker_project_spaces WHERE project_id='00000000-0000-4000-8000-000000000001' AND space_id='00000000-0000-4000-8000-000000000011';
COMMIT;
DO $$ BEGIN
    BEGIN
        DELETE FROM tracker_project_spaces WHERE project_id='00000000-0000-4000-8000-000000000001';
        SET CONSTRAINTS ALL IMMEDIATE;
        RAISE EXCEPTION 'last unlink accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
END $$;
UPDATE tracker_space_catalog SET state='archived' WHERE space_id='00000000-0000-4000-8000-000000000012';
DO $$ BEGIN
    IF (SELECT count(*) FROM tracker_project_spaces) <> 3 THEN RAISE EXCEPTION 'archive removed links'; END IF;
    IF (SELECT count(*) FROM projects) <> 3 THEN RAISE EXCEPTION 'archive removed projects'; END IF;
    BEGIN
        INSERT INTO tracker_project_spaces VALUES ('00000000-0000-4000-8000-000000000003','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000012',now());
        RAISE EXCEPTION 'new link to archived space accepted';
    EXCEPTION WHEN insufficient_privilege THEN NULL; END;
    BEGIN
        UPDATE tracker_space_admission_state SET active=false;
        RAISE EXCEPTION 'active admission disabled';
    EXCEPTION WHEN insufficient_privilege THEN NULL; END;
END $$;
-- A full replacement may transiently be empty inside its transaction.
BEGIN;
DELETE FROM tracker_project_spaces WHERE project_id='00000000-0000-4000-8000-000000000001';
INSERT INTO tracker_project_spaces VALUES ('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000011',now());
COMMIT;
UPDATE tracker_space_catalog SET state='active' WHERE space_id='00000000-0000-4000-8000-000000000012';
INSERT INTO tracker_project_spaces VALUES ('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000012',now());
