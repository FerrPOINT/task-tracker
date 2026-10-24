-- The identity owner takes the exclusive advisory lock before counting or
-- changing a fence. All reference writers take its shared form first, then the
-- epoch row. An old REPEATABLE READ snapshot cannot bypass a committed freeze.
CREATE TABLE tracker_identity_legacy_issuer (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    issuer text NOT NULL CHECK(length(btrim(issuer)) BETWEEN 1 AND 2048)
);
CREATE TABLE tracker_identity_fence_epoch (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    revision bigint NOT NULL DEFAULT 1 CHECK(revision>0)
);
INSERT INTO tracker_identity_fence_epoch(singleton) VALUES(true);
CREATE SEQUENCE tracker_identity_reference_revision MINVALUE 1;
CREATE TABLE tracker_identity_deletion_operations (
    operation_id uuid PRIMARY KEY,
    issuer text NOT NULL, subject text NOT NULL, inventory_revision uuid NOT NULL,
    request jsonb NOT NULL,
    state text NOT NULL CHECK(state IN ('frozen','released')),
    fence uuid, revision bigint NOT NULL CHECK(revision>0),
    reference_count bigint NOT NULL CHECK(reference_count>=0),
    CHECK((state='frozen')=(fence IS NOT NULL))
);
CREATE UNIQUE INDEX tracker_identity_active_freeze
    ON tracker_identity_deletion_operations(issuer,subject) WHERE state='frozen';

CREATE FUNCTION tracker_identity_write_lock() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_advisory_xact_lock_shared(714402216002::bigint);
    PERFORM singleton FROM tracker_identity_fence_epoch WHERE singleton FOR SHARE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'identity_fence_epoch_missing' USING ERRCODE='23514';
    END IF;
    PERFORM nextval('tracker_identity_reference_revision');
    RETURN NULL;
END;
$$;
CREATE FUNCTION tracker_identity_bind_legacy() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP<>'INSERT' THEN
        RAISE EXCEPTION 'identity_legacy_issuer_immutable' USING ERRCODE='23514';
    END IF;
    PERFORM pg_advisory_xact_lock(714402216002::bigint);
    UPDATE tracker_identity_fence_epoch SET revision=revision+1 WHERE singleton;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'identity_fence_epoch_missing' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER tracker_identity_bind_legacy BEFORE INSERT OR UPDATE OR DELETE
    ON tracker_identity_legacy_issuer FOR EACH ROW EXECUTE FUNCTION tracker_identity_bind_legacy();

-- A present local-only account is known noncentral. An orphan historical UUID
-- without a preserved binding is unknown, never guessed from UUID or email.
CREATE FUNCTION tracker_identity_local_ref(local_id text)
RETURNS TABLE(issuer text,subject text,resolved boolean) LANGUAGE plpgsql STABLE AS $$
DECLARE known_kind text;
BEGIN
    IF local_id IS NULL THEN RETURN; END IF;
    IF local_id !~ '^[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}$' THEN
        issuer:=NULL;subject:=local_id;resolved:=false;RETURN NEXT;RETURN;
    END IF;
    SELECT a.kind,i.issuer,a.central_subject INTO known_kind,issuer,subject
        FROM tracker_identity_aliases a LEFT JOIN tracker_identity_legacy_issuer i ON i.singleton
        WHERE a.local_user_id=local_id::uuid;
    IF FOUND THEN
        IF known_kind='central' THEN resolved:=true;RETURN NEXT;END IF;
        RETURN;
    END IF;
    issuer:=NULL;subject:=local_id;resolved:=false;RETURN NEXT;
END;
$$;

-- These are the audited scalar columns. Typed JSON lifecycle commands and
-- receipts require their own extractor before the owner API can acknowledge
-- the complete inventory; a scalar-only count must never be published as zero.
CREATE FUNCTION tracker_identity_scalar_refs(source text,document jsonb)
RETURNS TABLE(issuer text,subject text,resolved boolean) LANGUAGE plpgsql STABLE AS $$
DECLARE local_keys text[];subject_keys text[];key text;bound text;
BEGIN
    SELECT i.issuer INTO bound FROM tracker_identity_legacy_issuer i WHERE singleton;
    CASE source
      WHEN 'users' THEN
        IF NULLIF(document->>'central_sub','') IS NOT NULL THEN
            issuer:=bound;subject:=document->>'central_sub';resolved:=true;RETURN NEXT;
        END IF;
      WHEN 'tracker_identity_aliases' THEN
        IF document->>'kind'='central' THEN subject_keys:=ARRAY['central_subject']; END IF;
      WHEN 'projects' THEN local_keys:=ARRAY['owner_id'];
      WHEN 'project_members','issue_watchers','issue_votes','notification_user_settings',
           'oidc_identity','password_reset_token','totp' THEN local_keys:=ARRAY['user_id'];
      WHEN 'issues' THEN local_keys:=ARRAY['assignee_id','reporter_id'];
      WHEN 'attachments','comments','worklogs' THEN local_keys:=ARRAY['author_id'];
      WHEN 'issue_status_history' THEN local_keys:=ARRAY['changed_by_id'];
      WHEN 'notifications' THEN
        local_keys:=ARRAY['recipient_id','actor_id'];
        IF document->>'entity_type'='user' THEN
            RETURN QUERY SELECT * FROM tracker_identity_local_ref(document->>'entity_id');
        END IF;
      WHEN 'audit_logs' THEN
        local_keys:=ARRAY['actor_id'];
        IF document->>'entity_type'='user' THEN
            RETURN QUERY SELECT * FROM tracker_identity_local_ref(document->>'entity_id');
        END IF;
      WHEN 'project_repository_links','task_repository_links' THEN local_keys:=ARRAY['created_by'];
      WHEN 'issue_creation_receipts' THEN local_keys:=ARRAY['actor_id'];
      WHEN 'sdlc_tasks' THEN subject_keys:=ARRAY['owner_subject'];
      WHEN 'sdlc_analysis_reservation_leases','sdlc_pm_execution_leases' THEN subject_keys:=ARRAY['holder_subject'];
      WHEN 'sdlc_analysis_reservation_operations','sdlc_analysis_result_operations',
           'sdlc_critical_blocker_operations','sdlc_critical_pm_lifecycle_operations',
           'sdlc_critical_pm_returns','sdlc_critical_pm_reviews','sdlc_delivery_operations',
           'sdlc_delivery_proposal_operations','sdlc_draft_creations','sdlc_draft_source_enrollments',
           'sdlc_idempotency','sdlc_pm_completion_operations','sdlc_pm_feedback',
           'sdlc_pm_lease_operations','sdlc_pm_resume_inputs','sdlc_project_routing_operations',
           'sdlc_source_policy_operations' THEN subject_keys:=ARRAY['actor_subject'];
      WHEN 'tracker_space_owner_operations' THEN
        issuer:=document->>'owner_issuer';subject:=document->>'owner_subject';
        resolved:=(NULLIF(btrim(issuer),'') IS NOT NULL AND NULLIF(btrim(subject),'') IS NOT NULL);
        RETURN NEXT;
      ELSE RETURN;
    END CASE;
    FOREACH key IN ARRAY COALESCE(local_keys,ARRAY[]::text[]) LOOP
        RETURN QUERY SELECT * FROM tracker_identity_local_ref(document->>key);
    END LOOP;
    FOREACH key IN ARRAY COALESCE(subject_keys,ARRAY[]::text[]) LOOP
        IF document->>key IS NOT NULL THEN
            issuer:=bound;subject:=document->>key;resolved:=(length(btrim(subject))>0);RETURN NEXT;
        END IF;
    END LOOP;
END;
$$;

CREATE FUNCTION tracker_identity_scalar_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM tracker_identity_scalar_refs(TG_TABLE_NAME,to_jsonb(NEW)) r
        WHERE NOT r.resolved OR (NULLIF(btrim(r.issuer),'') IS NULL AND
            EXISTS(SELECT 1 FROM tracker_identity_legacy_issuer WHERE singleton))) THEN
        RAISE EXCEPTION 'identity_reference_unknown' USING ERRCODE='23514',
            CONSTRAINT='tracker_identity_reference_unknown';
    END IF;
    IF EXISTS(SELECT 1 FROM tracker_identity_scalar_refs(TG_TABLE_NAME,to_jsonb(NEW)) r
        JOIN tracker_identity_deletion_operations f ON f.issuer=r.issuer AND f.subject=r.subject
        WHERE f.state='frozen') THEN
        RAISE EXCEPTION 'identity_deletion_fenced' USING ERRCODE='23514',
            CONSTRAINT='tracker_identity_deletion_fenced';
    END IF;
    RETURN NEW;
END;
$$;

DO $$ DECLARE source text; BEGIN
    FOREACH source IN ARRAY ARRAY[
        'users','tracker_identity_aliases','projects','project_members','issues',
        'comments','worklogs','attachments','issue_status_history','issue_watchers','issue_votes',
        'notifications','notification_user_settings','audit_logs','totp','password_reset_token','oidc_identity',
        'project_repository_links','task_repository_links','issue_creation_receipts',
        'sdlc_tasks','sdlc_analysis_reservation_leases','sdlc_pm_execution_leases',
        'sdlc_analysis_reservation_operations','sdlc_analysis_result_operations',
        'sdlc_critical_blocker_operations','sdlc_critical_pm_lifecycle_operations',
        'sdlc_critical_pm_returns','sdlc_critical_pm_reviews','sdlc_delivery_operations',
        'sdlc_delivery_proposal_operations','sdlc_draft_creations','sdlc_draft_source_enrollments',
        'sdlc_idempotency','sdlc_pm_completion_operations','sdlc_pm_feedback',
        'sdlc_pm_lease_operations','sdlc_pm_resume_inputs','sdlc_project_routing_operations',
        'sdlc_source_policy_operations','tracker_space_owner_operations'
    ] LOOP
        EXECUTE format('CREATE TRIGGER a_identity_reference_lock BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH STATEMENT EXECUTE FUNCTION tracker_identity_write_lock()',source);
        EXECUTE format('CREATE TRIGGER z_identity_reference_guard BEFORE INSERT OR UPDATE ON %I FOR EACH ROW EXECUTE FUNCTION tracker_identity_scalar_write_guard()',source);
    END LOOP;
END;
$$;
