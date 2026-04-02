use super::*;

#[test]
fn test_validate_cron_expression() {
    assert!(validate_cron_expression("0 * * * *")); // Hourly
    assert!(validate_cron_expression("0 0 * * *")); // Daily at midnight
    assert!(validate_cron_expression("*/5 * * * *")); // Every 5 minutes
    assert!(validate_cron_expression("0 2 * * 1-5")); // 2 AM, Mon-Fri

    assert!(!validate_cron_expression("invalid"));
    assert!(!validate_cron_expression("0 * * *")); // Missing one field
}

#[test]
fn test_cron_job_creation() {
    let job = CronJob::new(
        "testapp".to_string(),
        "0 * * * *".to_string(),
        "echo 'hello'".to_string(),
    )
    .unwrap();

    assert_eq!(job.app, "testapp");
    assert_eq!(job.schedule, "0 * * * *");
    assert_eq!(job.command, "echo 'hello'");
}
