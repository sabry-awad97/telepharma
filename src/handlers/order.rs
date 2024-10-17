use chrono::Utc;
use sqlx::PgPool;
use teloxide::Bot;
use teloxide::{prelude::*, types::Message};

use crate::db::models::Medicine;

pub async fn place_order(bot: Bot, msg: Message, pool: PgPool) -> ResponseResult<()> {
    let user_id = msg.from.unwrap().id.to_string();

    // Simplified: Assume we're always ordering medicine with ID 1
    let medicine_id = 1;
    let quantity = 2;

    let order = sqlx::query_as::<_, Medicine>("SELECT * FROM medicines WHERE id = $1")
        .bind(medicine_id)
        .fetch_one(&pool)
        .await;

    if let Ok(order) = order {
        if order.stock >= quantity {
            // Reduce stock and create order
            if let Err(e) = sqlx::query("UPDATE medicines SET stock = stock - $1 WHERE id = $2")
                .bind(quantity)
                .bind(medicine_id)
                .execute(&pool)
                .await
            {
                log::error!("Failed to update stock: {}", e);
                bot.send_message(msg.chat.id, "Failed to update stock")
                    .await?;
                return Ok(());
            }

            let now = Utc::now().naive_utc();
            let order_id: i32 = rand::random();
            if let Err(e) = sqlx::query("INSERT INTO orders (id, user_id, medicine_id, quantity, status, created_at) VALUES ($1, $2, $3, $4, 'pending', $5)")
                    .bind(order_id) 
                    .bind(&user_id)
                    .bind(medicine_id)
                    .bind(quantity)
                    .bind(now)
                    .execute(&pool)
                    .await
                {
                    log::error!("Failed to create order: {}", e);
                    bot.send_message(msg.chat.id, "Failed to create order").await?;
                    return Ok(());
                }

            bot.send_message(msg.chat.id, "Order placed successfully")
                .await?;
        } else {
            bot.send_message(msg.chat.id, "Insufficient stock").await?;
        }
    } else {
        bot.send_message(msg.chat.id, "Medicine not found").await?;
    }

    Ok(())
}
