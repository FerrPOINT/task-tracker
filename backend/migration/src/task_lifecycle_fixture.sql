        INSERT INTO users(id,email,username,display_name,password_hash,central_sub)
        VALUES('10000000-0000-4000-8000-000000000001','operator@example.test','operator','Operator','fixture','10000000-0000-4000-8000-000000000001');
        BEGIN; SET CONSTRAINTS ALL DEFERRED;
        INSERT INTO projects(id,key,name,owner_id,default_board_id)
        VALUES('20000000-0000-4000-8000-000000000001','CORE','Core','10000000-0000-4000-8000-000000000001','30000000-0000-4000-8000-000000000001');
        INSERT INTO boards(id,project_id,name,columns)
        VALUES('30000000-0000-4000-8000-000000000001','20000000-0000-4000-8000-000000000001','Core','[]');
        COMMIT;
        INSERT INTO sdlc_instance(singleton,instance_id) VALUES(true,'40000000-0000-4000-8000-000000000001');
        INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,description,reporter_id,priority,labels,position,time_spent_seconds)
        SELECT ('50000000-0000-4000-8000-'||lpad(n::text,12,'0'))::uuid,
            '20000000-0000-4000-8000-000000000001','CORE-'||n,'Task',
            ('00000000-0000-0000-0000-'||lpad(n::text,12,'0'))::uuid,'Preserve '||n,'Unicode: кириллица',
            '10000000-0000-4000-8000-000000000001','Medium','[]',n,123
        FROM generate_series(1,4) AS n;
