use super::{EmailNotification, EmailSender, SmtpEmailSender, render_notification};
use shared::EmailConfig;

#[test]
fn notification_template_escapes_dynamic_html_and_includes_plain_text_fallback() {
    let notification = EmailNotification {
        recipient_address: "recipient@example.test".to_string(),
        recipient_name: Some("Ada & <Admin> \"O'Neil\"".to_string()),
        subject: "Update: <ready> & \"safe\"".to_string(),
        body: "Open <the> & confirm \"it\" 'now'".to_string(),
        action_url: Some("https://example.test/?a=1&b=<two>\"'".to_string()),
    };

    let rendered = render_notification(&notification);

    assert!(
        rendered
            .html
            .contains("Ada &amp; &lt;Admin&gt; &quot;O&#39;Neil&quot;")
    );
    assert!(
        rendered
            .html
            .contains("Update: &lt;ready&gt; &amp; &quot;safe&quot;")
    );
    assert!(
        rendered
            .html
            .contains("Open &lt;the&gt; &amp; confirm &quot;it&quot; &#39;now&#39;")
    );
    assert!(
        rendered
            .html
            .contains("https://example.test/?a=1&amp;b=&lt;two&gt;&quot;&#39;")
    );
    assert_eq!(
        rendered.plain_text,
        "Update: <ready> & \"safe\"\n\nOpen <the> & confirm \"it\" 'now'\n\nOpen: https://example.test/?a=1&b=<two>\"'"
    );
}

#[tokio::test]
async fn disabled_sender_is_noop_success_without_network() {
    let config = EmailConfig::default();
    assert!(!config.enabled);

    let sender = SmtpEmailSender::new(&config);
    assert!(!sender.is_enabled());

    let notification = EmailNotification {
        recipient_address: "nobody@example.test".to_string(),
        recipient_name: None,
        subject: "Nothing".to_string(),
        body: "This should not be sent.".to_string(),
        action_url: None,
    };

    let result = sender.send_notification(&notification).await;
    assert!(result.is_ok());
}
