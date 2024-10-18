use dotenvy::dotenv;
use dptree::case;
use envconfig::Envconfig;
use sqlx::PgPool;
use teloxide::{
    dispatching::{
        dialogue::{self, InMemStorage},
        Dispatcher, UpdateFilterExt,
    },
    prelude::*,
    types::{KeyboardButton, KeyboardMarkup, Me, ReplyMarkup},
    utils::command::BotCommands,
};

pub mod services;
pub mod utils;

type Error = Box<dyn std::error::Error + Send + Sync>;

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
    Start(String),
    #[command(description = "Check the pharmacy inventory.")]
    Inventory,
    #[command(description = "Place a medicine order.")]
    Order,
    #[command(description = "Display the main menu.")]
    Menu,
    #[command(description = "Display help information about available commands.")]
    Help,
    #[command(description = "Send an anonymous message to a pharmacist.")]
    Message,
}

impl Command {
    async fn show_in_message(&self, bot: &Bot, chat_id: ChatId) -> ResponseResult<Message> {
        bot.send_message(chat_id, Self::descriptions().to_string())
            .await
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub enum State {
    #[default]
    Start,
    WriteToPharmacist {
        id: ChatId,
    },
}

pub type MyDialogue = Dialogue<State, InMemStorage<State>>;

#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Medicine {
    pub id: i32,
    pub name: String,
    pub stock: i32,
    pub expiry_date: chrono::NaiveDate,
}

#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Order {
    pub id: i32,
    pub user_id: String,
    pub medicine_id: i32,
    pub quantity: i32,
    pub status: String,
    pub created_at: chrono::NaiveDate,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    log::info!("Starting the pharmacy bot...");
    dotenv().ok();

    let config = Config::init_from_env().unwrap();
    let pool = PgPool::connect(&config.database_url).await?;

    let bot = Bot::new(config.telegram_bot_token);

    let handler =
        dialogue::enter::<Update, InMemStorage<State>, State, _>()
            .branch(
                Update::filter_message()
                    .branch(dptree::entry().filter_command::<Command>().endpoint(answer)),
            )
            .branch(Update::filter_message().branch(
                case![State::WriteToPharmacist { id }].endpoint(send_message_to_pharmacist),
            ))
            .branch(Update::filter_message().endpoint(handle_message));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![pool, InMemStorage::<State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    log::info!("Shutting down gracefully");
    Ok(())
}

async fn answer(
    bot: Bot,
    msg: Message,
    cmd: Command,
    pool: PgPool,
    dialogue: MyDialogue,
    me: Me,
) -> Result<(), Error> {
    match cmd {
        Command::Start(start_param) => {
            if start_param.is_empty() {
                // Regular start command
                log::info!("Received start command");
                bot.send_message(msg.chat.id, "Welcome to the pharmacy bot!")
                    .await?;
            } else {
                // Deep link with pharmacist ID
                match start_param.parse::<i64>() {
                    Ok(id) => {
                        bot.send_message(msg.chat.id, "Send your message to the pharmacist:")
                            .await?;
                        dialogue
                            .update(State::WriteToPharmacist { id: ChatId(id) })
                            .await?;
                    }
                    Err(_) => {
                        bot.send_message(msg.chat.id, "Invalid link!").await?;
                    }
                }
            }
        }
        Command::Message => {
            let message_link = format!("{}?start={}", me.tme_url(), msg.chat.id);
            bot.send_message(
                msg.chat.id,
                format!(
                    "Share this link to receive anonymous messages: {}",
                    message_link
                ),
            )
            .await?;
        }
        Command::Inventory => {
            log::info!("Received inventory command");
            list_inventory(bot, msg, pool).await?;
        }
        Command::Order => {
            log::info!("Received order command");
            place_order(bot, msg, pool).await?;
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

async fn send_message_to_pharmacist(
    bot: Bot,
    id: ChatId,
    msg: Message,
    dialogue: MyDialogue,
) -> Result<(), Error> {
    if let Some(text) = msg.text() {
        let sent_result = bot
            .send_message(id, format!("You have a new anonymous message:\n\n{}", text))
            .await;

        if sent_result.is_ok() {
            bot.send_message(msg.chat.id, "Message sent to the pharmacist!")
                .await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Error sending message. The pharmacist may have blocked the bot.",
            )
            .await?;
        }
        dialogue.exit().await?;
    } else {
        bot.send_message(msg.chat.id, "Please send a text message.")
            .await?;
    }
    Ok(())
}

async fn handle_message(bot: Bot, msg: Message, pool: PgPool) -> Result<(), Error> {
    if let Some(text) = msg.text() {
        match text {
            "📋 Check Inventory" => list_inventory(bot, msg, pool).await?,
            "🛒 Place Order" => place_order(bot, msg, pool).await?,
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

async fn list_inventory(bot: Bot, msg: Message, pool: PgPool) -> ResponseResult<()> {
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

            let now = chrono::Utc::now().naive_utc();
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
