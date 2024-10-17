use dotenvy::dotenv;
use envconfig::Envconfig;
use teloxide::{prelude::*, utils::command::BotCommands};

#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "TELEGRAM_BOT_TOKEN")]
    telegram_bot_token: String,
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
async fn main() -> Result<(), std::io::Error> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    log::info!("Starting the pharmacy bot...");
    dotenv().ok();

    let config = Config::init_from_env().unwrap();
    let bot = Bot::new(config.telegram_bot_token);

    Command::repl(bot, answer).await;
    Ok(())
}

async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Start => {
            log::info!("Received start command");
            bot.send_message(msg.chat.id, "Welcome to the pharmacy bot!")
                .await?;
        }
        Command::Inventory => {
            log::info!("Received inventory command");
            bot.send_message(msg.chat.id, "Inventory command").await?;
        }
        Command::Order => {
            log::info!("Received order command");
            bot.send_message(msg.chat.id, "Order command").await?;
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
