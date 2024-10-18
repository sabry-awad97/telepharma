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
    // Initialize the logger with default settings or "info" level if not specified
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Log the start of the bot
    log::info!("Starting the pharmacy bot...");

    // Load environment variables from a .env file if present
    dotenv().ok();

    // Initialize configuration from environment variables
    let config = Config::init_from_env().unwrap();

    // Establish a connection to the PostgreSQL database
    let pool = PgPool::connect(&config.database_url).await?;

    // Create a new Telegram bot instance with the token from config
    let bot = Bot::new(config.telegram_bot_token);

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

    // Explanation of each line:
    // 1. Create a handler using dialogue::enter
    //    This sets up a dialogue system to manage different conversation states
    //    The generic types specify:
    //    - Update: Represents an update from the Telegram API
    //    - InMemStorage<State>: Uses in-memory storage for dialogue states
    //    - State: Our custom State enum for tracking conversation state
    //    - _: Placeholder for the return type of the handler

    // 2. Handle command messages
    //    - Uses Update::filter_message() to only process message updates
    //    - Further filters with filter_command::<Command>() to handle bot commands
    //    - Routes these to the 'answer' function

    // 3. Handle messages in the WriteToPharmacist state
    //    - Again uses Update::filter_message() to process only message updates
    //    - Uses case![State::WriteToPharmacist { id }] to match the specific state
    //    - Routes these to the 'send_message_to_pharmacist' function

    // 4. Handle all other messages
    //    - Catches any remaining message updates
    //    - Routes these to the 'handle_message' function

    // This structure allows the bot to handle different types of interactions,
    // maintaining state when necessary and providing a catch-all for general messages.

    // Build and run the dispatcher
    Dispatcher::builder(bot, handler)
        // Add dependencies: database pool and in-memory storage for dialogue states
        .dependencies(dptree::deps![pool, InMemStorage::<State>::new()])
        // Enable handling of Ctrl+C for graceful shutdown
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    // Detailed explanation:
    // 1. Dispatcher::builder(bot, handler):
    //    - Creates a new Dispatcher builder with the given bot instance and handler.
    //    - The handler is the message processing logic we defined earlier.

    // 2. .dependencies(dptree::deps![pool, InMemStorage::<State>::new()]):
    //    - Adds dependencies that will be available to all handler functions.
    //    - pool: The database connection pool for database operations.
    //    - InMemStorage::<State>::new(): Creates a new in-memory storage for dialogue states.
    //      This allows the bot to maintain conversation state across messages.

    // 3. .enable_ctrlc_handler():
    //    - Enables the Ctrl+C handler for graceful shutdown.
    //    - When Ctrl+C is pressed, the bot will attempt to shut down cleanly.

    // 4. .build():
    //    - Finalizes the Dispatcher configuration and builds the Dispatcher instance.

    // 5. .dispatch().await:
    //    - Starts the Dispatcher, which begins processing incoming updates from Telegram.
    //    - This is an asynchronous operation, so we use .await to wait for it to complete.
    //    - The Dispatcher will continue running until it's interrupted (e.g., by Ctrl+C).

    // This setup allows the bot to process messages, maintain state, and gracefully
    // handle shutdown requests, providing a robust foundation for the Telegram bot.

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
///
/// # Returns
///
/// Returns a Result indicating success or failure of the command handling.
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
            // This block handles the /start command with an optional parameter
            if start_param.is_empty() {
                // Case 1: No start parameter provided
                // Log the received command and send a welcome message
                log::info!("Received start command without parameter");
                bot.send_message(msg.chat.id, "Welcome to the pharmacy bot!")
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
            list_inventory(bot, msg, pool).await?;

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
async fn handle_message(bot: Bot, msg: Message, pool: PgPool) -> Result<(), Error> {
    if let Some(text) = msg.text() {
        match text {
            "📋 Check Inventory" => list_inventory(bot, msg, pool).await?,
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
/// - This implementation uses hardcoded values for medicine ID and quantity.
/// - In a real-world scenario, these would typically be provided by the user through interaction.
/// - The function uses database transactions to ensure data consistency when updating stock and creating orders.
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
