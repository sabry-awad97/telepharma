use dotenvy::dotenv;
use envconfig::Envconfig;
use sqlx::PgPool;
use teloxide::{
    dispatching::{Dispatcher, UpdateFilterExt},
    prelude::*,
    types::{KeyboardButton, KeyboardMarkup, ReplyMarkup},
    utils::command::BotCommands,
};

#[path = "db/mod.rs"]
pub mod db;

#[path = "handlers/mod.rs"]
pub mod handlers;

#[path = "services/mod.rs"]
pub mod services;

#[path = "utils/mod.rs"]
pub mod utils;

#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "TELEGRAM_BOT_TOKEN")]
    telegram_bot_token: String,

    #[envconfig(from = "DATABASE_URL")]
    database_url: String,
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
    #[command(description = "Display the main menu.")]
    Menu,
    #[command(description = "Display help information about available commands.")]
    Help,
}

impl Command {
    async fn show_in_message(&self, bot: &Bot, chat_id: ChatId) -> ResponseResult<Message> {
        bot.send_message(chat_id, Self::descriptions().to_string())
            .await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    log::info!("Starting the pharmacy bot...");
    dotenv().ok();

    let config = Config::init_from_env().unwrap();
    let pool = db::init_db(&config.database_url).await.unwrap();

    let bot = Bot::new(config.telegram_bot_token);

    let handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(answer))
        .branch(dptree::entry().endpoint(handle_message));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![pool])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

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
            handlers::inventory::list_inventory(bot, msg, pool).await?;
        }
        Command::Order => {
            log::info!("Received order command");
            handlers::order::place_order(bot, msg, pool).await?;
        }
        Command::Menu => {
            log::info!("Received menu command");
            let keyboard = KeyboardMarkup::new(vec![
                vec![KeyboardButton::new("📋 Check Inventory")],
                vec![KeyboardButton::new("🛒 Place Order")],
                vec![KeyboardButton::new("❓ Help")],
            ])
            .resize_keyboard()
            .one_time_keyboard();

            let menu_text = "Welcome to the Pharmacy Bot! Please choose an option:";

            bot.send_message(msg.chat.id, menu_text)
                .reply_markup(ReplyMarkup::Keyboard(keyboard))
                .await?;
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
                "/menu \\- Display the main menu",
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

async fn handle_message(bot: Bot, msg: Message, pool: PgPool) -> ResponseResult<()> {
    if let Some(text) = msg.text() {
        match text {
            "📋 Check Inventory" => handlers::inventory::list_inventory(bot, msg, pool).await?,
            "🛒 Place Order" => handlers::order::place_order(bot, msg, pool).await?,
            "❓ Help" => {
                Command::Help.show_in_message(&bot, msg.chat.id).await?;
            }
            _ => {
                bot.send_message(msg.chat.id, "I don't understand that command. Please use the menu or type /help for available commands.").await?;
            }
        }
    }
    Ok(())
}
