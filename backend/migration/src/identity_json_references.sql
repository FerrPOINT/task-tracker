-- Only lifecycle records below are typed identity-bearing JSON. Product text,
-- arbitrary settings, labels, field values and repository snapshots are never
-- searched for strings resembling users. Container names are a contract list,
-- not a recursive scan of every JSON key.
CREATE FUNCTION tracker_identity_json_refs(document jsonb,inherited_issuer text,depth integer DEFAULT 0)
RETURNS TABLE(issuer text,subject text,resolved boolean) LANGUAGE plpgsql STABLE AS $$
DECLARE item record;bound text;issuer_key text;
BEGIN
    IF document IS NULL OR document='null'::jsonb THEN RETURN;END IF;
    IF depth>32 THEN issuer:=NULL;subject:=NULL;resolved:=false;RETURN NEXT;RETURN;END IF;
    IF jsonb_typeof(document)='array' THEN
        FOR item IN SELECT value FROM jsonb_array_elements(document) LOOP
            RETURN QUERY SELECT * FROM tracker_identity_json_refs(item.value,inherited_issuer,depth+1);
        END LOOP;
    ELSIF jsonb_typeof(document)='object' THEN
        FOREACH issuer_key IN ARRAY ARRAY['issuer','actor_issuer','owner_issuer','responsible_issuer'] LOOP
            IF document ? issuer_key AND (jsonb_typeof(document->issuer_key) IS DISTINCT FROM 'string'
                OR NULLIF(btrim(document->>issuer_key),'') IS NULL) THEN
                issuer:=NULL;subject:=NULL;resolved:=false;RETURN NEXT;RETURN;
            END IF;
        END LOOP;
        bound:=COALESCE(document->>'issuer',document->>'actor_issuer',inherited_issuer);
        FOR item IN SELECT * FROM jsonb_each(document) LOOP
            IF item.key IN ('subject','owner_subject','author_subject','actor_subject','holder_subject',
                'producer_subject','machine_subject','verifier_subject','responsible_subject','central_subject') THEN
                IF item.key='subject' AND item.value='null'::jsonb THEN
                    issuer:=NULL;subject:=NULL;resolved:=false;RETURN NEXT;
                END IF;
                IF item.value<>'null'::jsonb THEN
                    issuer:=COALESCE(CASE item.key
                        WHEN 'owner_subject' THEN document->>'owner_issuer'
                        WHEN 'actor_subject' THEN document->>'actor_issuer'
                        WHEN 'responsible_subject' THEN document->>'responsible_issuer'
                        ELSE NULL END,bound);subject:=item.value#>>'{}';
                    resolved:=(jsonb_typeof(item.value)='string' AND NULLIF(btrim(subject),'') IS NOT NULL);
                    RETURN NEXT;
                END IF;
            ELSIF item.key IN ('owner_id','actor_id','user_id','author_id','created_by','responsible_user_id') THEN
                IF item.value<>'null'::jsonb THEN
                    RETURN QUERY SELECT * FROM tracker_identity_local_ref(item.value#>>'{}');
                END IF;
            ELSIF item.key IN ('assignment','binding','lease','execution','current','previous','next',
                'lifecycle','routing','policy','identity','actor','actor_identity','original_actor',
                'owner','responsible','command','readback','result','payload','state','next_state',
                'next_execution','before_state','after_state','before_execution','after_execution',
                'before_receipt','after_receipt','before_blocker','after_blocker','blocker','blocking',
                'packet','returned','receipt','completion','proposal','confirmation','confirmations',
                'answer','answers','question','questions','requirements','revisions','evidence','task',
                'review','initial_pm_assignment','initial_routing','analysis_intent','snapshot',
                'witness','execution_witness','renewal_witness','readback_witness','reservation','create_spec') THEN
                RETURN QUERY SELECT * FROM tracker_identity_json_refs(item.value,bound,depth+1);
            END IF;
        END LOOP;
    END IF;
END;
$$;

CREATE FUNCTION tracker_identity_row_refs(source text,document jsonb)
RETURNS TABLE(issuer text,subject text,resolved boolean) LANGUAGE plpgsql STABLE AS $$
DECLARE payloads text[];key text;bound text;
BEGIN
    RETURN QUERY SELECT * FROM tracker_identity_scalar_refs(source,document);
    SELECT i.issuer INTO bound FROM tracker_identity_legacy_issuer i WHERE singleton;
    CASE source
      WHEN 'users','tracker_identity_aliases','projects','project_members','issues','attachments',
           'comments','worklogs','issue_status_history','issue_watchers','issue_votes','notifications',
           'notification_user_settings','audit_logs','oidc_identity','password_reset_token','totp',
           'project_repository_links','task_repository_links','issue_creation_receipts',
           'sdlc_analysis_reservation_leases','sdlc_project_routing_operations','sdlc_source_policy_operations' THEN
        payloads:=ARRAY[]::text[];
      WHEN 'sdlc_tasks' THEN payloads:=ARRAY['state'];
      WHEN 'sdlc_analysis_intents','sdlc_analysis_results','sdlc_answers','sdlc_assignments',
           'sdlc_confirmations','sdlc_delivery_assignments','sdlc_delivery_proposals',
           'sdlc_delivery_reconciliations','sdlc_delivery_results','sdlc_delivery_run_bindings',
           'sdlc_draft_source_enrollments','sdlc_evidence','sdlc_options','sdlc_outbox',
           'sdlc_pm_feedback','sdlc_project_routing_revisions','sdlc_question_versions',
           'sdlc_requirements','sdlc_reservation_capacity','sdlc_source_policy_revisions',
           'sdlc_task_routing_generations','sdlc_task_routing_snapshots','sdlc_task_source_bindings' THEN
        payloads:=ARRAY['payload'];
      WHEN 'sdlc_critical_blockers' THEN payloads:=ARRAY['payload','previous_receipt'];
      WHEN 'sdlc_analysis_reservation_operations','sdlc_analysis_result_operations',
           'sdlc_critical_blocker_operations','sdlc_delivery_operations','sdlc_delivery_proposal_operations',
           'sdlc_draft_creations','sdlc_idempotency','sdlc_pm_completion_operations',
           'sdlc_pm_completions','sdlc_pm_execution_leases','sdlc_pm_executions','sdlc_pm_lease_operations' THEN
        payloads:=ARRAY['result'];
      WHEN 'sdlc_critical_pm_lifecycle_operations' THEN
        payloads:=ARRAY['command','result','before_state','after_state','before_execution',
            'after_execution','before_blocker','after_blocker'];
      WHEN 'sdlc_critical_pm_returns' THEN
        payloads:=ARRAY['command','before_receipt','after_receipt','after_blocker'];
      WHEN 'sdlc_critical_pm_reviews' THEN
        payloads:=ARRAY['command','result','before_state','after_state','before_execution','after_execution','after_blocker'];
      WHEN 'sdlc_delivery_heads' THEN payloads:=ARRAY['execution'];
      WHEN 'sdlc_delivery_projection_permits' THEN payloads:=ARRAY['next_state','next_execution'];
      WHEN 'sdlc_analysis_state_write_permits','sdlc_pm_state_write_permits' THEN payloads:=ARRAY['next_state'];
      WHEN 'sdlc_pm_resume_inputs' THEN payloads:=ARRAY['command','result'];
      WHEN 'tracker_execution_space_grants' THEN payloads:=ARRAY['original_actor'];
      WHEN 'tracker_namespace_bindings' THEN payloads:=ARRAY['command'];
      WHEN 'tracker_project_lifecycle_operations' THEN payloads:=ARRAY['actor_identity','command'];
      WHEN 'tracker_project_space_operations' THEN payloads:=ARRAY['actor_identity','requested_change'];
      WHEN 'tracker_project_space_outbox' THEN payloads:=ARRAY['actor_identity','command','readback'];
      WHEN 'tracker_space_owner_operations' THEN payloads:=ARRAY['command','readback'];
      ELSE RAISE EXCEPTION 'identity_reference_source_unknown';
    END CASE;
    FOREACH key IN ARRAY payloads LOOP
        RETURN QUERY SELECT * FROM tracker_identity_json_refs(document->key,bound);
    END LOOP;
END;
$$;

-- Replace the foundation scalar predicate for all existing trigger bindings.
CREATE OR REPLACE FUNCTION tracker_identity_scalar_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM tracker_identity_row_refs(TG_TABLE_NAME,to_jsonb(NEW)) r
        WHERE NOT r.resolved OR (NULLIF(btrim(r.issuer),'') IS NULL AND
            EXISTS(SELECT 1 FROM tracker_identity_legacy_issuer WHERE singleton))) THEN
        RAISE EXCEPTION 'identity_reference_unknown' USING ERRCODE='23514',
            CONSTRAINT='tracker_identity_reference_unknown';
    END IF;
    IF EXISTS(SELECT 1 FROM tracker_identity_row_refs(TG_TABLE_NAME,to_jsonb(NEW)) r
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
        'sdlc_analysis_intents','sdlc_analysis_results','sdlc_answers','sdlc_assignments',
        'sdlc_confirmations','sdlc_critical_blockers','sdlc_delivery_assignments','sdlc_delivery_proposals',
        'sdlc_delivery_reconciliations','sdlc_delivery_results','sdlc_delivery_run_bindings',
        'sdlc_evidence','sdlc_options','sdlc_outbox','sdlc_pm_completions','sdlc_pm_executions',
        'sdlc_project_routing_revisions','sdlc_question_versions','sdlc_requirements',
        'sdlc_reservation_capacity','sdlc_source_policy_revisions','sdlc_task_routing_generations',
        'sdlc_task_routing_snapshots','sdlc_task_source_bindings','sdlc_delivery_heads',
        'sdlc_delivery_projection_permits','sdlc_analysis_state_write_permits','sdlc_pm_state_write_permits',
        'tracker_execution_space_grants','tracker_namespace_bindings','tracker_project_lifecycle_operations',
        'tracker_project_space_operations','tracker_project_space_outbox'
    ] LOOP
        EXECUTE format('CREATE TRIGGER a_identity_reference_lock BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH STATEMENT EXECUTE FUNCTION tracker_identity_write_lock()',source);
        EXECUTE format('CREATE TRIGGER z_identity_reference_guard BEFORE INSERT OR UPDATE ON %I FOR EACH ROW EXECUTE FUNCTION tracker_identity_scalar_write_guard()',source);
    END LOOP;
END;
$$;
