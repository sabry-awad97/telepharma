use crate::{
    db::models::Medicine,
    utils::{escape_markdown, format_date},
};
use chrono::Utc;
use futures::future;
use sqlx::PgPool;
use teloxide::prelude::*;
use tokio_cron_scheduler::{Job, JobScheduler};

/// Schedules notifications for expiring medicines.
///
/// This function sets up a scheduled job to check for expiring medicines and send notifications.
/// It uses the `tokio_cron_scheduler` crate to create a job that runs daily at 8:00 AM.
///
/// Parameters:
/// - `pool`: A PostgreSQL connection pool for database operations.
/// - `bot`: A Telegram Bot instance for sending notifications.
/// - `pharmacy_group_chat_id`: The ChatId of the pharmacy group where notifications will be sent.
///
/// The function performs the following steps:
/// 1. Creates a new JobScheduler instance.
/// 2. Defines a new asynchronous job that runs daily at 8:00 AM.
/// 3. The job calls `check_and_notify_expiring_medicines` function.
/// 4. Adds the job to the scheduler and starts it.
///
/// Returns:
/// - `Ok(())` if the job is successfully scheduled and started.
/// - `Err(Box<dyn std::error::Error>)` if any step fails.
pub async fn schedule_notifications(
    pool: PgPool,
    bot: Bot,
    pharmacy_group_chat_id: ChatId,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a new JobScheduler
    let sched = JobScheduler::new().await?;

    // Define the job to run every 5 seconds
    let job = Job::new_async("*/5 * * * * *", move |_uuid, _l| {
        let bot = bot.clone();
        let pool = pool.clone();
        let chat_id = pharmacy_group_chat_id;
        Box::pin(async move {
            match check_and_notify_expiring_medicines(&pool, &bot, chat_id).await {
                Ok(_) => log::info!("Expiring medicines check completed successfully"),
                Err(e) => log::error!("Error checking expiring medicines: {}", e),
            }
        })
    })
    .map_err(|e| {
        log::error!("Failed to create job: {}", e);
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    // Add the job to the scheduler
    sched.add(job).await.map_err(|e| {
        log::error!("Failed to add job to scheduler: {}", e);
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    // Start the scheduler in a separate task
    tokio::spawn(async move {
        if let Err(e) = sched.start().await {
            log::error!("Scheduler error: {}", e);
        }
    });

    log::info!("Notification scheduler started successfully");
    Ok(())
}

/// Checks for expiring medicines and sends notifications.
///
/// This function performs the following steps:
/// 1. Fetches a list of medicines that are expiring soon from the database.
/// 2. For each expiring medicine, sends a notification to the specified chat.
///
/// Parameters:
/// - `pool`: A reference to the PostgreSQL connection pool.
/// - `bot`: A reference to the Telegram Bot instance used to send messages.
/// - `chat_id`: The ID of the chat where notifications will be sent.
///
/// Returns:
/// - `Ok(())` if all operations succeed.
/// - `Err(Box<dyn std::error::Error>)` if any step fails.
///
/// Note: This function uses `?` operator to propagate errors from both
/// `fetch_expiring_medicines` and `send_expiry_notification` functions.
async fn check_and_notify_expiring_medicines(
    pool: &PgPool,
    bot: &Bot,
    chat_id: ChatId,
) -> Result<(), Box<dyn std::error::Error>> {
    // Fetch the list of expiring medicines
    let medicines = fetch_expiring_medicines(pool).await?;

    // Create a vector to store all the notification futures
    let notification_futures: Vec<_> = medicines
        .iter()
        .map(|medicine| send_expiry_notification(bot, chat_id, medicine))
        .collect();

    // Execute all notification futures concurrently
    let results = future::join_all(notification_futures).await;

    // Check if any notifications failed
    for result in results {
        if let Err(e) = result {
            log::error!("Failed to send notification: {}", e);
        }
    }

    // Return Ok if all operations succeeded
    Ok(())
}

/// Fetches medicines that are expiring within the next 6 months from the database.
///
/// This function queries the database for all medicines whose expiry date is less than or equal to
/// 6 months from the current date and time. It uses the following parameters:
///
/// - `pool`: A reference to the PostgreSQL connection pool.
///
/// The function performs the following steps:
/// 1. Calculates the date 6 months from now.
/// 2. Constructs an SQL query to select all columns from the 'medicines' table where the expiry_date
///    is less than or equal to the calculated future date.
/// 3. Binds the future date to the query parameter.
/// 4. Executes the query and fetches all matching rows, mapping them to `Medicine` structs.
///
/// Returns a `Result` containing either:
/// - `Ok(Vec<Medicine>)`: A vector of `Medicine` structs representing the medicines expiring within 6 months.
/// - `Err(sqlx::Error)`: An error if the database query fails.
async fn fetch_expiring_medicines(pool: &PgPool) -> Result<Vec<Medicine>, sqlx::Error> {
    let six_months_from_now = Utc::now() + chrono::Duration::days(180);
    sqlx::query_as::<_, Medicine>("SELECT * FROM medicines WHERE expiry_date <= $1")
        .bind(six_months_from_now.naive_utc())
        .fetch_all(pool)
        .await
}

/// Sends a notification about an expiring medicine to the specified chat.
///
/// This function is responsible for notifying the pharmacy group about medicines
/// that are about to expire. It takes the following parameters:
///
/// - `bot`: A reference to the Telegram Bot instance used to send messages.
/// - `chat_id`: The ID of the chat (likely a group chat) where the notification will be sent.
/// - `medicine`: A reference to the Medicine struct containing information about the expiring medicine.
///
/// The function constructs a formatted message with the medicine's name and sends it to the specified chat.
/// It returns a Result, which will be Ok(()) if the message was sent successfully, or an error if there was a problem.
async fn send_expiry_notification(
    bot: &Bot,
    chat_id: ChatId,
    medicine: &Medicine,
) -> Result<(), teloxide::RequestError> {
    // Calculate days until expiry
    let days_until_expiry = (medicine.expiry_date - Utc::now().date_naive()).num_days();

    // Escape special characters for Markdown
    let escaped_name = escape_markdown(&medicine.name);
    let formatted_date = format_date(medicine.expiry_date);
    // Construct the notification message with Markdown formatting
    let message = format!(
        "⚠️ *Medicine Expiry Alert*\n\n\
        *Name:* `{}`\n\
        *Expiry Date:* `{}`\n\
        *Days until expiry:* `{}`\n\
        *Quantity:* `{}`\n\
        Please check and take appropriate action\\.",
        escaped_name, formatted_date, days_until_expiry, medicine.stock,
    );

    // Send the message to the specified chat with Markdown parsing
    bot.send_message(chat_id, message)
        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
        .await?;

    // If we've reached this point, the message was sent successfully
    Ok(())
}
