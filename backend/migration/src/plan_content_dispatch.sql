-- Keep plan rows under the existing comments policy without losing the narrow
-- native comments/attachments insertion port introduced after decomposition.
CREATE OR REPLACE FUNCTION tracker_authorize_project_content(table_name text,operation text,previous jsonb,desired jsonb)
RETURNS void LANGUAGE plpgsql VOLATILE AS $$
BEGIN
 IF table_name IN ('issue_plan_revisions','issue_plan_operations') THEN
  PERFORM tracker_authorize_project_content_before_decomposition('comments',operation,previous,desired);
 ELSIF table_name IN ('comments','attachments') AND operation='INSERT'
    AND NULLIF(current_setting('pdlc.tracker.native_caller',true),'') IS NOT NULL THEN
  PERFORM tracker_authorize_native_task_content(table_name,operation,desired);
 ELSE
  PERFORM tracker_authorize_project_content_before_configured_comments(table_name,operation,previous,desired);
 END IF;
END $$;
