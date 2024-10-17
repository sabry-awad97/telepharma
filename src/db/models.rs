use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct Medicine {
    pub id: i32,
    pub name: String,
    pub stock: i32,
    pub expiry_date: NaiveDate,
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct Order {
    pub id: i32,
    pub user_id: String,
    pub medicine_id: i32,
    pub quantity: i32,
    pub status: String,
    pub created_at: NaiveDate,
}
