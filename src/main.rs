use dotenvy::dotenv;
use envconfig::Envconfig;
use sqlx::PgPool;
use teloxide::{prelude::*, utils::command::BotCommands};

#[path = "db/mod.rs"]
pub mod db;

#[path = "handlers/mod.rs"]
pub mod handlers;

#[path = "services/mod.rs"]
pub mod services;

#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "TELEGRAM_BOT_TOKEN")]
    telegram_bot_token: String,

    #[envconfig(from = "DATABASE_URL")]
    database_url: String,

    #[envconfig(from = "PHARMACY_GROUP_CHAT_ID")]
    pharmacy_group_chat_id: i64,
}

#[derive(BotCommands, Debug, Clone)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
enum Command {
    #[command(description = "Start interacting with the pharmacy bot.")]
    Start,
    #[command(description = "Check the pharmacy inventory.")]
    Inventory,
    #[command(description = "Place a medicine order.")]
    Order,
    #[command(description = "Display help information about available commands.")]
    Help,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    log::info!("Starting the pharmacy bot...");
    dotenv().ok();

    let config = Config::init_from_env().unwrap();
    let pool = db::init_db(&config.database_url).await.unwrap();

    // Run migrations
    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
        log::error!("Failed to run migrations: {}", e);
    }

    let bot = Bot::new(config.telegram_bot_token);
    // Schedule notifications
    let pharmacy_group_chat_id = ChatId(config.pharmacy_group_chat_id);
    services::schedule_notifications(pool.clone(), bot.clone(), pharmacy_group_chat_id).await?;

    // Create a shutdown signal handler
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        tx.send(()).ok();
    });

    // Run the bot with graceful shutdown
    tokio::select! {
        _ = Command::repl(bot, move |cx, msg, cmd| {
            let pool = pool.clone();
            answer(cx, msg, cmd, pool)
        }) => {
            log::info!("Bot stopped");
        }
        _ = rx => {
            log::info!("Received shutdown signal");
        }
    }

    log::info!("Shutting down gracefully");
    Ok(())
}

async fn answer(bot: Bot, msg: Message, cmd: Command, pool: PgPool) -> ResponseResult<()> {
    println!("Received command: {:?}", pool);
    match cmd {
        Command::Start => {
            log::info!("Received start command");
            bot.send_message(msg.chat.id, "Welcome to the pharmacy bot!")
                .await?;
        }
        Command::Inventory => {
            log::info!("Received inventory command");
            handlers::inventory::list_inventroy(bot, msg, pool).await?;
        }
        Command::Order => {
            log::info!("Received order command");
            handlers::order::place_order(bot, msg, pool).await?;
        }
        Command::Help => {
            log::info!("Received help command");
            let help_text = [
                "*Pharmacy Bot Help*",
                "",
                "Here are the available commands:",
                "",
                "/start \\- Start interacting with the pharmacy bot",
                "/inventory \\- Check the pharmacy inventory",
                "/order \\- Place a medicine order",
                "/help \\- Display this help information",
                "",
                "To use a command, simply type it or tap on it\\.",
            ]
            .join("\n");

            bot.send_message(msg.chat.id, help_text)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
        }
    };
    Ok(())
}
