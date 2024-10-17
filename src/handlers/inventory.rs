use sqlx::PgPool;
use teloxide::{prelude::*, types::Message};

use crate::db::models::Medicine;

pub async fn list_inventroy(bot: Bot, msg: Message, pool: PgPool) -> ResponseResult<()> {
    log::info!("Listing inventory");
    let medicines = sqlx::query_as::<_, Medicine>("SELECT * FROM medicines")
        .fetch_all(&pool)
        .await
        .unwrap_or_else(|_| vec![]);

    if medicines.is_empty() {
        bot.send_message(msg.chat.id, "No medicines found in the inventory")
            .await?;
        return Ok(());
    }

    let message = medicines
        .iter()
        .map(|medicine| {
            format!(
                "🏥 *{}*\n   Stock: {} units\n   Expires: {}",
                medicine.name,
                medicine.stock,
                medicine.expiry_date.format("%d %b %Y")
            )
        })
        .collect::<Vec<String>>()
        .join("\n\n");

    let formatted_message = format!("*Available medicines:*\n\n{}", message);

    bot.send_message(msg.chat.id, formatted_message).await?;

    Ok(())
}
