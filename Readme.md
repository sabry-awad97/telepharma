# TelePharma Bot

TelePharma Bot is a Telegram bot designed to manage a pharmacy inventory and handle medicine orders. It provides an easy-to-use interface for users to check inventory and place orders for medicines.

## Features

- Check pharmacy inventory
- Place medicine orders
- User-friendly command interface

## Commands

- `/start` - Start interacting with the pharmacy bot
- `/inventory` - Check the pharmacy inventory
- `/order` - Place a medicine order
- `/help` - Display help information about available commands

## Technical Stack

- Rust programming language
- Teloxide library for Telegram Bot API
- SQLx for database operations
- PostgreSQL database

## Setup

1. Clone the repository
2. Set up a PostgreSQL database
3. Create a `.env` file with the following variables:

   ```sh
   TELEGRAM_BOT_TOKEN=your_bot_token_here
   DATABASE_URL=your_database_url_here
   ```

4. Run database migrations:

   ```sh
   sqlx migrate run
   ```

5. Build and run the bot:

   ```sh
   cargo run
   ```

## Project Structure

- `src/main.rs`: Entry point of the application
- `src/db/`: Database-related code and models
- `src/handlers/`: Command handlers for the bot

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License.
