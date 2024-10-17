use chrono::NaiveDate;

/// Helper function to format the date
///
/// This function takes a `NaiveDate` and formats it as a string in the "dd-mm-yyyy" format.
///
/// # Arguments
///
/// * `date` - A `NaiveDate` object representing the date to be formatted
///
/// # Returns
///
/// A `String` containing the formatted date
pub fn format_date(date: NaiveDate) -> String {
    date.format("%d-%m-%Y").to_string()
}

/// Helper function to escape special characters for Markdown
///
/// This function takes a string and escapes special characters that have
/// special meaning in Markdown syntax. This is useful when sending messages
/// that contain Markdown formatting to ensure that certain characters are
/// treated as literal text rather than Markdown syntax.
///
/// # Arguments
///
/// * `text` - A string slice containing the text to be escaped
///
/// # Returns
///
/// A `String` with all Markdown special characters escaped
pub fn escape_markdown(text: &str) -> String {
    text.replace(|c: char| "._*[]()~`>#+-=|{}.!".contains(c), "\\")
}
