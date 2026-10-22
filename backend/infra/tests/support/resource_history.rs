//! Historical text must disappear from listings and counts when its project is denied.
use infra::repos::SeaOrmRepositories;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use shared::{UserId, space_access::SpaceRef};
pub async fn verify(
    repos: &SeaOrmRepositories,
    db: &DatabaseConnection,
    user: UserId,
    a: &SpaceRef,
    b: &SpaceRef,
) {
    db.execute_unprepared("CREATE TABLE issues(id uuid PRIMARY KEY,project_id uuid NOT NULL);CREATE TABLE audit_logs(id uuid PRIMARY KEY,actor_id uuid NOT NULL,action text NOT NULL,entity_type text NOT NULL,entity_id uuid,metadata jsonb NOT NULL,created_at timestamptz NOT NULL);CREATE TABLE notifications(id uuid PRIMARY KEY,recipient_id uuid NOT NULL,event_type text NOT NULL,entity_type text NOT NULL,entity_id uuid,actor_id uuid,title text NOT NULL,body text,is_read boolean NOT NULL,read_at timestamptz,action_url text,metadata jsonb NOT NULL,created_at timestamptz NOT NULL);INSERT INTO issues VALUES('00000000-0000-4000-8000-000000000111','00000000-0000-4000-8000-000000000001'),('00000000-0000-4000-8000-000000000112','00000000-0000-4000-8000-000000000002')").await.unwrap();
    for (kind, resource, title) in [
        ("project", Some(super::id("1")), "project AB"),
        ("issue", Some(super::id("112")), "private B text"),
        ("user", Some(user.as_uuid()), "global user audit"),
        ("unclassified", None, "unclassified secret"),
    ] {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO audit_logs VALUES($1,$2,'test.event',$3,$4,$5,now())",
            [
                uuid::Uuid::new_v4().into(),
                user.as_uuid().into(),
                kind.into(),
                resource.into(),
                serde_json::json!({"text":title}).into(),
            ],
        ))
        .await
        .unwrap();
        db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO notifications(id,recipient_id,event_type,entity_type,entity_id,title,body,is_read,metadata,created_at) VALUES($1,$2,'test.event',$3,$4,$5,$5,false,'{}',now())",[uuid::Uuid::new_v4().into(),user.as_uuid().into(),kind.into(),resource.into(),title.into()])).await.unwrap();
    }
    // Another recipient and a read message must not inflate unread counts.
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO notifications(id,recipient_id,event_type,entity_type,entity_id,title,is_read,metadata,created_at) VALUES($1,$2,'test.event','issue','00000000-0000-4000-8000-000000000111','already read',true,'{}',now()),($3,$4,'test.event','issue','00000000-0000-4000-8000-000000000111','other recipient',false,'{}',now())",[uuid::Uuid::new_v4().into(),user.as_uuid().into(),uuid::Uuid::new_v4().into(),uuid::Uuid::new_v4().into()])).await.unwrap();
    let audit_a = repos
        .audit_logs
        .list_space_visible(&[a.clone()], 100, 0)
        .await
        .unwrap();
    assert_eq!(audit_a.len(), 2);
    assert!(
        !audit_a
            .iter()
            .any(|e| e.metadata.to_string().contains("private B")
                || e.entity_type.as_ref() == "unclassified")
    );
    let audit_b = repos
        .audit_logs
        .list_space_visible(&[b.clone()], 100, 0)
        .await
        .unwrap();
    assert_eq!(audit_b.len(), 3);
    let global = repos
        .audit_logs
        .list_space_visible(&[], 100, 0)
        .await
        .unwrap();
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].entity_type.as_ref(), "user");
    let (page, count) = repos
        .notifications
        .list_space_visible(user, &[a.clone()], false, 1, 0)
        .await
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(count, 1);
    assert_eq!(page[0].title.as_ref(), "project AB");
    let (page, count) = repos
        .notifications
        .list_space_visible(user, &[a.clone()], false, 1, 1)
        .await
        .unwrap();
    assert!(page.is_empty());
    assert_eq!(count, 1);
    let (page, count) = repos
        .notifications
        .list_space_visible(user, &[a.clone(), b.clone()], false, 50, 0)
        .await
        .unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(count, 2);
    assert!(page.iter().any(|n| n.title.as_ref() == "private B text"));
    let (page, count) = repos
        .notifications
        .list_space_visible(user, &[a.clone()], true, 50, 0)
        .await
        .unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(count, 1);
    let (page, count) = repos
        .notifications
        .list_space_visible(user, &[], true, 50, 0)
        .await
        .unwrap();
    assert!(page.is_empty());
    assert_eq!(count, 0);
}
