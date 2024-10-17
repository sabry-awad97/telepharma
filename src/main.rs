use dotenvy::dotenv;
use envconfig::Envconfig;
use teloxide::{prelude::Requester, respond, types::Message, Bot};

#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "TELEGRAM_BOT_TOKEN")]
    telegram_bot_token: String,
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv().ok();

    let config = Config::init_from_env().unwrap();
    let bot = Bot::new(config.telegram_bot_token);

    teloxide::repl(bot, |bot: Bot, message: Message| async move {
        if let Some(text) = message.text() {
            bot.send_message(message.chat.id, text).await?;
        }
        respond(())
    })
    .await;
    Ok(())
}
