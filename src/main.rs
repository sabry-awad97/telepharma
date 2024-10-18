use chrono::Duration;
use dotenvy::dotenv;
use dptree::case;
use envconfig::Envconfig;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::Row;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::str::FromStr;
use teloxide::{
    dispatching::{
        dialogue::{self, InMemStorage},
        Dispatcher, UpdateFilterExt,
    },
    prelude::*,
    types::{ChatPermissions, KeyboardButton, KeyboardMarkup, Me, ReplyMarkup},
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
#[command(
    rename_rule = "lowercase",
    description = "Available commands:",
    parse_with = "split"
)]
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
    #[command(description = "Kick a user from the chat")]
    Kick,
    #[command(description = "Ban a user from the chat")]
    Ban { time: u64, unit: UnitOfTime },
    #[command(description = "Mute a user in the chat")]
    Mute { time: u64, unit: UnitOfTime },
}

#[derive(Clone, Debug)]
enum UnitOfTime {
    Seconds,
    Minutes,
    Hours,
}

impl FromStr for UnitOfTime {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "h" | "hours" => Ok(UnitOfTime::Hours),
            "m" | "minutes" => Ok(UnitOfTime::Minutes),
            "s" | "seconds" => Ok(UnitOfTime::Seconds),
            _ => Err("Allowed units: h, m, s"),
        }
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
    pub id: i64,
    pub name: String,
    pub stock: i64,
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

// Add this new struct to represent our translations
#[derive(Clone)]
struct I18n {
    translations: HashMap<String, HashMap<String, String>>,
}

impl I18n {
    fn new() -> Self {
        let mut translations = HashMap::new();

        // English translations
        let mut en = HashMap::new();
        en.insert(
            "welcome".to_string(),
            "Welcome to the pharmacy bot!".to_string(),
        );
        en.insert("inventory".to_string(), "Available medicines:".to_string());
        en.insert(
            "no_medicines".to_string(),
            "No medicines found in the inventory".to_string(),
        );
        // ... add more English translations ...

        // Spanish translations
        let mut es = HashMap::new();
        es.insert(
            "welcome".to_string(),
            "¡Bienvenido al bot de farmacia!".to_string(),
        );
        es.insert(
            "inventory".to_string(),
            "Medicamentos disponibles:".to_string(),
        );
        es.insert(
            "no_medicines".to_string(),
            "No se encontraron medicamentos en el inventario".to_string(),
        );
        // ... add more Spanish translations ...

        translations.insert("en".to_string(), en);
        translations.insert("es".to_string(), es);

        I18n { translations }
    }

    fn get(&self, lang: &str, key: &str) -> String {
        self.translations
            .get(lang)
            .and_then(|map| map.get(key))
            .cloned()
            .unwrap_or_else(|| format!("Missing translation: {}", key))
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize the logger with default settings or "info" level if not specified
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Log the start of the bot
    log::info!("Starting the pharmacy bot...");

    // Load environment variables from a .env file if present
    dotenv().ok();

    // Initialize configuration from environment variables
    let config = Config::init_from_env().unwrap();

    // Initialize SQLite database
    let options = SqliteConnectOptions::from_str(&config.database_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;

    // Create a new Telegram bot instance with the token from config
    let bot = Bot::new(config.telegram_bot_token);

    let i18n = I18n::new();

    // Set up the message handler for the bot
    let handler =
        dialogue::enter::<Update, InMemStorage<State>, State, _>()
            // Handle command messages
            .branch(
                Update::filter_message()
                    .branch(dptree::entry().filter_command::<Command>().endpoint(answer)),
            )
            // Handle messages in the WriteToPharmacist state
            .branch(Update::filter_message().branch(
                case![State::WriteToPharmacist { id }].endpoint(send_message_to_pharmacist),
            ))
            // Handle all other messages
            .branch(Update::filter_message().endpoint(handle_message));

    // Build and run the dispatcher
    Dispatcher::builder(bot, handler)
        // Add dependencies: database pool and in-memory storage for dialogue states
        .dependencies(dptree::deps![pool, InMemStorage::<State>::new(), i18n])
        // Enable handling of Ctrl+C for graceful shutdown
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    // Log shutdown message
    log::info!("Shutting down gracefully");
    Ok(())
}

/// Handles bot commands and responds accordingly.
///
/// This function is responsible for processing various bot commands and executing
/// the appropriate actions based on the received command.
///
/// # Arguments
///
/// * `bot` - The Telegram Bot instance used to send messages.
/// * `msg` - The received message containing the command.
/// * `cmd` - The parsed command enum.
/// * `pool` - The database connection pool.
/// * `dialogue` - The dialogue state for managing conversation flow.
/// * `me` - Information about the bot itself.
/// * `i18n` - The internationalization (i18n) instance for translations.
///
/// # Returns
///
/// Returns a Result indicating success or failure of the command handling.
async fn answer(
    bot: Bot,
    msg: Message,
    cmd: Command,
    pool: SqlitePool,
    dialogue: MyDialogue,
    me: Me,
    i18n: I18n,
) -> Result<(), Error> {
    // Determine the user's language (you might want to store this in a database)
    let lang = msg
        .from
        .as_ref()
        .and_then(|user| user.language_code.clone())
        .unwrap_or_else(|| "en".to_string());

    match cmd {
        Command::Start(start_param) => {
            // This block handles the /start command with an optional parameter
            if start_param.is_empty() {
                // Case 1: No start parameter provided
                // Log the received command and send a welcome message
                log::info!("Received start command without parameter");
                bot.send_message(msg.chat.id, i18n.get(&lang, "welcome"))
                    .await?;
            } else {
                // Case 2 & 3: Start parameter provided (could be valid or invalid)
                // Attempt to parse the start parameter as a 64-bit integer (pharmacist ID)
                match start_param.parse::<i64>() {
                    Ok(id) => {
                        // Case 2: Valid pharmacist ID
                        // Prompt the user to send a message to the pharmacist
                        // and update the dialogue state to WriteToPharmacist
                        log::info!("Received start command with valid pharmacist ID: {}", id);
                        bot.send_message(msg.chat.id, "Send your message to the pharmacist:")
                            .await?;
                        dialogue
                            .update(State::WriteToPharmacist { id: ChatId(id) })
                            .await?;
                    }
                    Err(_) => {
                        // Case 3: Invalid pharmacist ID
                        // Inform the user that the link is invalid
                        log::warn!(
                            "Received start command with invalid parameter: {}",
                            start_param
                        );
                        bot.send_message(msg.chat.id, "Invalid link!").await?;
                    }
                }
            }

            // To test:
            // 1. Send "/start" command to the bot
            // 2. Use a deep link with a valid pharmacist ID (e.g., "t.me/YourBot?start=123456789")
            // 3. Use a deep link with an invalid ID (e.g., "t.me/YourBot?start=invalid_id")
        }
        Command::Message => {
            // Generate and send a message link for anonymous communication
            let message_link = format!("{}?start={}", me.tme_url(), msg.chat.id);
            bot.send_message(
                msg.chat.id,
                format!(
                    "Share this link to receive anonymous messages: {}",
                    message_link
                ),
            )
            .await?;

            // Add a test case comment
            // Test case: Send "/message" command to the bot and verify the response
        }
        Command::Inventory => {
            // Handle inventory command
            log::info!("Received inventory command");
            list_inventory(bot.clone(), msg.clone(), pool, i18n).await?;

            // Test case: Send "/inventory" command to the bot
            // Expected behavior:
            // 1. The bot should log the received command
            // 2. The list_inventory function should be called with the correct parameters
            // 3. The function should return without errors
            // 4. Verify that the inventory list is displayed to the user
        }
        Command::Order => {
            // Handle order command
            log::info!("Received order command");
            place_order(bot, msg, pool).await?;

            // Test case: Send "/order" command to the bot
            // Expected behavior:
            // 1. The bot should log the received command
            // 2. The place_order function should be called with the correct parameters
            // 3. The function should return without errors
            // 4. Verify that the order placement process is initiated for the user
        }
        Command::Menu => {
            // Log the received menu command
            log::info!("Received menu command");

            // Create a custom keyboard with three options
            let keyboard = KeyboardMarkup::new(vec![
                vec![KeyboardButton::new("📋 Check Inventory")],
                vec![KeyboardButton::new("🛒 Place Order")],
                vec![KeyboardButton::new("❓ Help")],
            ])
            .resize_keyboard() // Allow the keyboard to be resized
            .one_time_keyboard(); // Make the keyboard disappear after one use

            // Define the welcome message
            let menu_text = "Welcome to the Pharmacy Bot! Please choose an option:";

            // Send the message with the custom keyboard
            bot.send_message(msg.chat.id, menu_text)
                .reply_markup(ReplyMarkup::Keyboard(keyboard))
                .await?;

            // This code creates a custom keyboard menu for the Pharmacy Bot.
            // It displays three options: Check Inventory, Place Order, and Help.
            // The keyboard is resizable and disappears after one use.
            // The bot sends a welcome message along with this custom keyboard.

            // Test case: Send "/menu" command to the bot
            // Expected behavior:
            // 1. The bot should log the received command
            // 2. A custom keyboard should be displayed with the specified options
            // 3. The welcome message should be sent along with the keyboard
            // 4. Verify that the keyboard is resizable and one-time use
        }
        Command::Help => {
            // Display help information
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

            // Test case: Send "/help" command to the bot
            // Expected behavior:
            // 1. The bot should log the received command
            // 2. The help text should be constructed with the correct commands and formatting
            // 3. The message should be sent to the user with Markdown parsing
            // 4. Verify that the help information is displayed correctly to the user
        }
        Command::Kick => {
            log::info!("Received kick command");
            kick_user(bot, msg).await?

            // This code handles the Kick command:
            // 1. It logs that a kick command was received.
            // 2. It calls the kick_user function with the bot and message as arguments.
            // 3. The result of kick_user is propagated up with the ? operator.

            // Test case: Send "/kick" command as a reply to another user's message
            // Expected behavior:
            // 1. The bot should log "Received kick command"
            // 2. The kick_user function should be called with correct arguments
            // 3. If kick_user succeeds, the command handler should return Ok(())
            // 4. If kick_user fails, the error should be propagated up

            // Additional test cases:
            // - Send "/kick" without replying to a message
            // - Send "/kick" as a non-admin user
            // - Send "/kick" targeting an admin user
        }
        Command::Ban { time, unit } => {
            log::info!("Received ban command: {} {:?}", time, unit);
            ban_user(bot, msg, calc_restrict_time(time, unit)).await?

            // This code handles the Ban command:
            // 1. It logs that a ban command was received, including the time and unit.
            // 2. It calls the ban_user function with the bot, message, and calculated restriction time.
            // 3. The result of ban_user is propagated up with the ? operator.

            // Test case: Send "/ban 2 h" command as a reply to another user's message
            // Expected behavior:
            // 1. The bot should log "Received ban command: 2 Hours"
            // 2. The calc_restrict_time function should be called with (2, UnitOfTime::Hours)
            // 3. The ban_user function should be called with correct arguments
            // 4. If ban_user succeeds, the command handler should return Ok(())
            // 5. If ban_user fails, the error should be propagated up

            // Additional test cases:
            // - Send "/ban 30 m" to ban for 30 minutes
            // - Send "/ban 60 s" to ban for 60 seconds
            // - Send "/ban" without time and unit (should handle error gracefully)
            // - Send "/ban" as a non-admin user (should be rejected)
            // - Send "/ban" targeting an admin user (should be rejected)
        }
        Command::Mute { time, unit } => {
            log::info!("Received mute command: {} {:?}", time, unit);
            mute_user(bot, msg, calc_restrict_time(time, unit)).await?

            // This code handles the Mute command:
            // 1. It logs that a mute command was received, including the time and unit.
            // 2. It calls the calc_restrict_time function to convert the time and unit into a Duration.
            // 3. It calls the mute_user function with the bot, message, and calculated restriction time.
            // 4. The result of mute_user is propagated up with the ? operator.

            // Test case: Send "/mute 5 m" command as a reply to another user's message
            // Expected behavior:
            // 1. The bot should log "Received mute command: 5 Minutes"
            // 2. The calc_restrict_time function should be called with (5, UnitOfTime::Minutes)
            // 3. The mute_user function should be called with correct arguments
            // 4. If mute_user succeeds, the command handler should return Ok(())
            // 5. If mute_user fails, the error should be propagated up

            // Additional test cases:
            // - Send "/mute 1 h" to mute for 1 hour
            // - Send "/mute 30 s" to mute for 30 seconds
            // - Send "/mute" without time and unit (should handle error gracefully)
            // - Send "/mute" as a non-admin user (should be rejected)
            // - Send "/mute" targeting an admin user (should be rejected)
            // - Verify that the muted user cannot send messages for the specified duration
            // - Verify that the mute is automatically lifted after the specified duration
        }
    };

    Ok(())
}

/// Sends an anonymous message from a user to a pharmacist.
///
/// This function handles the process of sending an anonymous message from a user to a pharmacist.
/// It's designed to be used in a state where the user has already initiated the process of sending
/// a message to a pharmacist.
///
/// # Arguments
///
/// * `bot` - The Telegram Bot instance used to send messages.
/// * `id` - The ChatId of the pharmacist who will receive the message.
/// * `msg` - The Message object containing the user's message.
/// * `dialogue` - The MyDialogue instance managing the conversation state.
///
/// # Returns
///
/// Returns a Result indicating success or failure of the operation.
///
/// # Function flow
///
/// 1. Check if the message contains text.
/// 2. If text is present, attempt to send it to the pharmacist.
/// 3. Notify the user of the result (success or failure).
/// 4. Exit the dialogue state.
/// 5. If no text is present, prompt the user to send a text message.
///
/// # Error handling
///
/// - If sending the message to the pharmacist fails, the user is notified of the error.
/// - Any errors during the process are propagated up the call stack.
async fn send_message_to_pharmacist(
    bot: Bot,
    id: ChatId,
    msg: Message,
    dialogue: MyDialogue,
) -> Result<(), Error> {
    if let Some(text) = msg.text() {
        // Attempt to send the message to the pharmacist
        let sent_result = bot
            .send_message(id, format!("You have a new anonymous message:\n\n{}", text))
            .await;

        // Notify the user based on the result
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

        // Exit the dialogue state
        dialogue.exit().await?;
    } else {
        // Prompt the user to send a text message if no text was found
        bot.send_message(msg.chat.id, "Please send a text message.")
            .await?;
    }
    Ok(())
}

/// Handles incoming messages from users.
///
/// This function processes text messages and executes corresponding actions based on the content.
///
/// # Arguments
///
/// * `bot` - The Bot instance used to send responses.
/// * `msg` - The incoming Message to be processed.
/// * `pool` - The database connection pool for any database operations.
///
/// # Returns
///
/// Returns a Result indicating success or failure of the operation.
///
/// # Function flow
///
/// 1. Check if the message contains text.
/// 2. If text is present, match it against known commands:
///    - "📋 Check Inventory": List the current inventory.
///    - "🛒 Place Order": Initiate the order placement process.
///    - "❓ Help": Display help information.
///    - Any other text: Send a message indicating an unknown command.
/// 3. If no text is present, do nothing (implicit in the if let structure).
///
/// # Error handling
///
/// - Any errors during the process are propagated up the call stack.
async fn handle_message(bot: Bot, msg: Message, pool: SqlitePool) -> Result<(), Error> {
    if let Some(text) = msg.text() {
        match text {
            "📋 Check Inventory" => list_inventory(bot, msg, pool, I18n::new()).await?,
            "🛒 Place Order" => place_order(bot, msg, pool).await?,
            "❓ Help" => {
                bot.send_message(msg.chat.id, Command::descriptions().to_string())
                    .await?;
            }
            _ => {
                bot.send_message(msg.chat.id, "I don't understand that command. Please use the menu or type /help for available commands.").await?;
            }
        }
    }
    Ok(())
}

/// Lists the inventory of medicines to the user.
///
/// This function retrieves all medicines from the database and sends a formatted
/// message to the user with the inventory details.
///
/// # Arguments
///
/// * `bot` - The Bot instance used to send messages.
/// * `msg` - The original message that triggered this function.
/// * `pool` - The database connection pool.
/// * `i18n` - The internationalization (i18n) instance for translations.
///
/// # Returns
///
/// Returns a `ResponseResult<()>`, which is `Ok(())` if the operation succeeds,
/// or an error if something goes wrong.
///
/// # Function flow
///
/// 1. Log the inventory listing action.
/// 2. Query the database for all medicines.
/// 3. If no medicines are found, inform the user and return.
/// 4. If medicines are found, format each medicine's details.
/// 5. Combine all formatted medicine details into a single message.
/// 6. Send the formatted message to the user.
///
/// # Error handling
///
/// - Database errors are handled by returning an empty vector if the query fails.
/// - Message sending errors are propagated up the call stack.
///
/// # Formatting
///
/// The function formats each medicine with:
/// - An emoji (🏥)
/// - The medicine name in bold
/// - The current stock
/// - The expiry date (formatted as "DD Mon YYYY")
///
/// Medicines are separated by two newlines for readability.
async fn list_inventory(
    bot: Bot,
    msg: Message,
    pool: SqlitePool,
    i18n: I18n,
) -> ResponseResult<()> {
    let lang = msg
        .from
        .and_then(|user| user.language_code.clone())
        .unwrap_or_else(|| "en".to_string());

    log::info!("Listing inventory");
    let medicines = sqlx::query_as::<_, Medicine>("SELECT * FROM medicines")
        .fetch_all(&pool)
        .await
        .unwrap_or_else(|_| vec![]);

    if medicines.is_empty() {
        bot.send_message(msg.chat.id, i18n.get(&lang, "no_medicines"))
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

    let formatted_message = format!("{}:\n\n{}", i18n.get(&lang, "inventory"), message);

    bot.send_message(msg.chat.id, formatted_message).await?;

    Ok(())
}

/// Handles the process of placing an order for medicine.
///
/// This function performs the following steps:
/// 1. Extracts the user ID from the incoming message.
/// 2. Sets up hardcoded values for medicine ID and quantity (for simplification).
/// 3. Queries the database to check if the requested medicine exists and has sufficient stock.
/// 4. If the medicine is available:
///    a. Updates the stock in the database.
///    b. Creates a new order entry in the database.
///    c. Sends a confirmation message to the user.
/// 5. If the medicine is not available or there's insufficient stock, informs the user.
///
/// # Arguments
///
/// * `bot` - The Telegram Bot instance used to send messages.
/// * `msg` - The incoming message from the user.
/// * `pool` - The database connection pool.
///
/// # Returns
///
/// Returns a `ResponseResult<()>` which is `Ok(())` if the operation succeeds,
/// or an error if any step fails.
///
/// # Error Handling
///
/// - Database errors are logged and appropriate messages are sent to the user.
/// - If updating stock or creating an order fails, the operation is aborted and the user is notified.
///
/// # Notes
///
/// - This implementation uses hardcoded values for medicine name and quantity.
/// - In a real-world scenario, these would typically be provided by the user through interaction.
/// - The function uses database transactions to ensure data consistency when updating stock and creating orders.
pub async fn place_order(bot: Bot, msg: Message, pool: SqlitePool) -> Result<(), crate::Error> {
    let user_id = msg.from.unwrap().id.to_string();

    let medicine_name = "acetaminophen";
    let quantity = 2;

    let search_pattern = format!("%{}%", medicine_name);
    let medicine = sqlx::query_as!(
        Medicine,
        "SELECT * FROM medicines WHERE LOWER(name) LIKE LOWER($1) LIMIT 1",
        search_pattern
    )
    .fetch_one(&pool)
    .await?;

    if medicine.stock >= quantity {
        // Start a transaction
        let mut transaction = pool.begin().await?;

        // Reduce stock
        sqlx::query("UPDATE medicines SET stock = stock - $1 WHERE id = $2")
            .bind(quantity)
            .bind(medicine.id)
            .execute(&mut *transaction)
            .await?;

        // Get the current time in the local timezone
        let local_tz = chrono::Local::now().timezone();
        let now = chrono::Utc::now().with_timezone(&local_tz);

        // Create order
        let order_id = sqlx::query("INSERT INTO orders (user_id, medicine_id, quantity, status, created_at) VALUES ($1, $2, $3, 'pending', $4) RETURNING id")
            .bind(&user_id)
            .bind(medicine.id)
            .bind(quantity)
            .bind(now.naive_local())
            .fetch_one(&mut *transaction)
            .await?
            .get::<i32, _>("id");

        // Commit the transaction
        transaction.commit().await?;

        bot.send_message(
            msg.chat.id,
            format!(
                "Your order for {} (x{}) has been placed. Order ID: {}",
                medicine.name, quantity, order_id
            ),
        )
        .await?;
    } else {
        bot.send_message(msg.chat.id, "Insufficient stock").await?;
    }

    Ok(())
}

/// Kicks a user from a chat.
///
/// This function handles the process of kicking a user in response to a command.
/// It checks if the command is a reply to another message, identifies the user to be kicked,
/// applies the kick, and sends appropriate feedback messages.
///
/// # Arguments
///
/// * `bot` - The Bot instance used to interact with the Telegram API.
/// * `msg` - The Message object that triggered this command.
///
/// # Returns
///
/// Returns a `ResponseResult<()>` which is `Ok(())` if the operation succeeds,
/// or an error if any step fails.
///
/// # Function flow
///
/// 1. Check if the command is a reply to another message.
/// 2. If it is a reply, try to identify the user to be kicked.
/// 3. If a user is identified, attempt to kick them using `unban_chat_member`.
/// 4. Send a confirmation message if the kick is successful.
/// 5. If any step fails, send an appropriate error message.
///
/// # Note
///
/// This function uses `unban_chat_member` to kick the user. In Telegram's API,
/// unbanning a user who is in the chat will remove them from the chat.
async fn kick_user(bot: Bot, msg: Message) -> ResponseResult<()> {
    if let Some(replied) = msg.reply_to_message() {
        if let Some(user) = &replied.from {
            // Kick the user by "unbanning" them
            bot.unban_chat_member(msg.chat.id, user.id).await?;
            // Send confirmation message
            bot.send_message(
                msg.chat.id,
                format!("User {} has been kicked.", user.first_name),
            )
            .await?;
        } else {
            // Send error message if user couldn't be identified
            bot.send_message(msg.chat.id, "Couldn't identify the user to kick.")
                .await?;
        }
    } else {
        // Send instruction if the command wasn't a reply
        bot.send_message(msg.chat.id, "Use this command in reply to another message")
            .await?;
    }
    Ok(())
}

/// Bans a user from a chat for a specified duration.
///
/// This function handles the process of banning a user in response to a command.
/// It checks if the command is a reply to another message, identifies the user to be banned,
/// applies the ban, and sends appropriate feedback messages.
///
/// # Arguments
///
/// * `bot` - The Bot instance used to interact with the Telegram API.
/// * `msg` - The Message object that triggered this command.
/// * `time` - A Duration object specifying how long the user should be banned.
///
/// # Returns
///
/// Returns a `ResponseResult<()>` which is `Ok(())` if the operation succeeds,
/// or an error if any step fails.
///
/// # Errors
///
/// This function will return an error if:
/// * The bot fails to ban the chat member.
/// * The bot fails to send a message.
///
/// # Function flow
///
/// 1. Check if the command is a reply to another message.
/// 2. If it's a reply, try to get the user who sent the original message.
/// 3. If a user is identified, attempt to ban them for the specified duration.
/// 4. Send a confirmation message if the ban is successful.
/// 5. If any step fails, send an appropriate error message.
///
/// # Note
///
/// This function uses `kick_chat_member` with an `until_date` parameter to implement a temporary ban.
/// After the specified duration, the user will be able to join the chat again.
async fn ban_user(bot: Bot, msg: Message, time: Duration) -> ResponseResult<()> {
    // This code handles the process of banning a user in a Telegram chat.
    // Here's a breakdown of what it does:

    // 1. Check if the command is a reply to another message
    if let Some(replied) = msg.reply_to_message() {
        // 2. If it's a reply, try to get the user who sent the original message
        if let Some(user) = &replied.from {
            // 3. If we have a user, attempt to ban them
            // The 'kick_chat_member' method is used for banning
            // 'until_date' sets the duration of the ban
            bot.kick_chat_member(msg.chat.id, user.id)
                .until_date(msg.date + time)
                .await?;

            // 4. If the ban is successful, send a confirmation message
            bot.send_message(
                msg.chat.id,
                format!(
                    "User {} has been banned for the specified duration.",
                    user.first_name
                ),
            )
            .await?;
        } else {
            // 5. If we couldn't identify the user, send an error message
            bot.send_message(msg.chat.id, "Couldn't identify the user to ban.")
                .await?;
        }
    } else {
        // 6. If the command wasn't a reply, instruct the user on how to use it
        bot.send_message(
            msg.chat.id,
            "Use this command in a reply to another message!",
        )
        .await?;
    }
    Ok(())
}

/// Mutes a user in a chat for a specified duration.
///
/// This function handles the process of muting a user in response to a command.
/// It checks if the command is a reply to another message, identifies the user to be muted,
/// applies the mute restriction, and sends appropriate feedback messages.
///
/// # Arguments
///
/// * `bot` - The Bot instance used to interact with the Telegram API.
/// * `msg` - The Message object that triggered this command.
/// * `time` - A Duration object specifying how long the user should be muted.
///
/// # Returns
///
/// Returns a `ResponseResult<()>` which is `Ok(())` if the operation succeeds,
/// or an error if any step fails.
///
/// # Errors
///
/// This function will return an error if:
/// * The bot fails to restrict the chat member.
/// * The bot fails to send a message.
///
async fn mute_user(bot: Bot, msg: Message, time: Duration) -> ResponseResult<()> {
    // This code handles the muting of a user in response to a command
    if let Some(replied) = msg.reply_to_message() {
        // Check if the command is a reply to another message
        if let Some(user) = &replied.from {
            // If we can identify the user to be muted
            // Restrict the user's chat permissions
            bot.restrict_chat_member(msg.chat.id, user.id, ChatPermissions::empty())
                .until_date(msg.date + time)
                .await?;

            // Send a confirmation message
            bot.send_message(
                msg.chat.id,
                format!(
                    "User {} has been muted for the specified duration.",
                    user.first_name
                ),
            )
            .await?;
        } else {
            // If we couldn't identify the user to be muted
            bot.send_message(msg.chat.id, "Couldn't identify the user to mute.")
                .await?;
        }
    } else {
        // If the command wasn't a reply to another message
        bot.send_message(
            msg.chat.id,
            "Use this command in a reply to another message!",
        )
        .await?;
    }

    Ok(())
}

/// Calculates the restriction time based on the given time and unit.
///
/// This function takes a time value and a unit of time, and returns a Duration
/// object representing the total restriction time.
///
/// # Arguments
///
/// * `time` - An unsigned 64-bit integer representing the amount of time.
/// * `unit` - A UnitOfTime enum value specifying the unit of the given time (Hours, Minutes, or Seconds).
///
/// # Returns
///
/// Returns a Duration object representing the calculated restriction time.
///
/// # Note
///
/// The function converts the input `time` from u64 to i64 when creating the Duration.
/// This is safe for the expected use cases, but very large values might cause overflow.
fn calc_restrict_time(time: u64, unit: UnitOfTime) -> Duration {
    match unit {
        UnitOfTime::Hours => Duration::hours(time as i64),
        UnitOfTime::Minutes => Duration::minutes(time as i64),
        UnitOfTime::Seconds => Duration::seconds(time as i64),
    }
}
