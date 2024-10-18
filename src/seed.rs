use chrono::NaiveDate;
use sqlx::PgPool;

#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Medicine {
    pub id: i32,
    pub name: String,
    pub stock: i32,
    pub expiry_date: chrono::NaiveDate,
}

#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Order {
    pub id: i32,
    pub user_id: String,
    pub medicine_id: i32,
    pub quantity: i32,
    pub status: String,
    pub created_at: chrono::NaiveDate,
}

fn get_seed_data() -> (Vec<Medicine>, Vec<Order>) {
    let seed_medicines = vec![
        Medicine {
            id: 1,
            name: "Aspirin".to_string(),
            stock: 500,
            expiry_date: NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
        },
        Medicine {
            id: 2,
            name: "Amoxicillin".to_string(),
            stock: 300,
            expiry_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        },
        Medicine {
            id: 3,
            name: "Lisinopril".to_string(),
            stock: 400,
            expiry_date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
        },
        Medicine {
            id: 4,
            name: "Levothyroxine".to_string(),
            stock: 250,
            expiry_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
        },
        Medicine {
            id: 5,
            name: "Metformin".to_string(),
            stock: 350,
            expiry_date: NaiveDate::from_ymd_opt(2025, 9, 30).unwrap(),
        },
        Medicine {
            id: 6,
            name: "Amlodipine".to_string(),
            stock: 200,
            expiry_date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
        },
        Medicine {
            id: 7,
            name: "Omeprazole".to_string(),
            stock: 450,
            expiry_date: NaiveDate::from_ymd_opt(2025, 7, 31).unwrap(),
        },
        Medicine {
            id: 8,
            name: "Albuterol".to_string(),
            stock: 150,
            expiry_date: NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
        },
        Medicine {
            id: 9,
            name: "Gabapentin".to_string(),
            stock: 300,
            expiry_date: NaiveDate::from_ymd_opt(2025, 5, 31).unwrap(),
        },
        Medicine {
            id: 10,
            name: "Metoprolol".to_string(),
            stock: 275,
            expiry_date: NaiveDate::from_ymd_opt(2024, 10, 31).unwrap(),
        },
    ];

    let seed_orders = vec![
        Order {
            id: 1,
            user_id: "user123".to_string(),
            medicine_id: 1,
            quantity: 2,
            status: "Delivered".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 5, 15).unwrap(),
        },
        Order {
            id: 2,
            user_id: "patient456".to_string(),
            medicine_id: 3,
            quantity: 1,
            status: "Shipped".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 6, 2).unwrap(),
        },
        Order {
            id: 3,
            user_id: "customer789".to_string(),
            medicine_id: 2,
            quantity: 3,
            status: "Processed".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 6, 10).unwrap(),
        },
        Order {
            id: 4,
            user_id: "client101".to_string(),
            medicine_id: 5,
            quantity: 1,
            status: "Pending".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 6, 12).unwrap(),
        },
        Order {
            id: 5,
            user_id: "user123".to_string(),
            medicine_id: 7,
            quantity: 2,
            status: "Delivered".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 5, 20).unwrap(),
        },
        Order {
            id: 6,
            user_id: "patient456".to_string(),
            medicine_id: 4,
            quantity: 1,
            status: "Shipped".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 6, 5).unwrap(),
        },
        Order {
            id: 7,
            user_id: "customer789".to_string(),
            medicine_id: 6,
            quantity: 2,
            status: "Processed".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 6, 11).unwrap(),
        },
        Order {
            id: 8,
            user_id: "client101".to_string(),
            medicine_id: 8,
            quantity: 1,
            status: "Pending".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 6, 13).unwrap(),
        },
        Order {
            id: 9,
            user_id: "user123".to_string(),
            medicine_id: 9,
            quantity: 3,
            status: "Delivered".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 5, 25).unwrap(),
        },
        Order {
            id: 10,
            user_id: "patient456".to_string(),
            medicine_id: 10,
            quantity: 1,
            status: "Shipped".to_string(),
            created_at: NaiveDate::from_ymd_opt(2023, 6, 7).unwrap(),
        },
    ];

    (seed_medicines, seed_orders)
}

pub async fn seed_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    let (medicines, orders) = get_seed_data();

    // Seed medicines
    for medicine in medicines {
        sqlx::query!(
            "INSERT INTO medicines (id, name, stock, expiry_date) VALUES ($1, $2, $3, $4)",
            medicine.id,
            medicine.name,
            medicine.stock,
            medicine.expiry_date
        )
        .execute(pool)
        .await?;
    }

    // Seed orders
    for order in orders {
        sqlx::query!(
            "INSERT INTO orders (id, user_id, medicine_id, quantity, status, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
            order.id,
            order.user_id,
            order.medicine_id,
            order.quantity,
            order.status,
            order.created_at
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    dotenvy::dotenv().ok();
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap()).await?;
    seed_database(&pool).await?;
    Ok(())
}
