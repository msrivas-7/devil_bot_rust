use lambda_http::{service_fn, Error, IntoResponse, Request};
use log::LevelFilter;
use serde_json::{json, Value};
use simple_logger::SimpleLogger;
use std::{env, process};

mod aws;
mod commands;

const BUNS_TABLE_NAME: &str = "BUNS_TABLE_NAME";
const DEVIL_BOT_TEST_CHANNEL_URL: &str = "DEVIL_BOT_TEST_CHANNEL_URL";

#[tokio::main]
async fn main() -> Result<(), Error> {
    SimpleLogger::new()
        .with_utc_timestamps()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();

    let func = service_fn(handler);
    lambda_http::run(func).await?;
    Ok(())
}

// This is the main event handler in the AWS Lambda. It parses the
// requests that were sent to the static endpoint behind our AWS
// API Gateway.
async fn handler(request: Request) -> Result<impl IntoResponse, Error> {
    let (_parts, body) = request.into_parts();
    let body: Value = serde_json::from_slice(&body)?;
    log::info!("{}", body);
    let challenge: String = intercept_challenge_request(&body).await;
    intercept_command(&body).await;

    Ok(json!({ "challenge": challenge }))
}

// When you create a Slack event subscription, your endpoint needs
// to respond to a challenge request with the challenge ID for
// the subscription to be successfully created.
// Read more here: https://api.slack.com/apis/connections/events-api
async fn intercept_challenge_request(body: &Value) -> String {
    let token: &str = body["token"].as_str().unwrap_or("invalid_token");
    let challenge: &str = body["challenge"].as_str().unwrap_or("invalid_challenge");
    let message_type: &str = body["type"].as_str().unwrap_or("invalid_type");

    if challenge == "invalid_challenge" {
        log::info!("Not a challenge request.");
    } else {
        let challenge_info: String = format!(
            "token: {}\nchallenge: {}\ntype: {}",
            token, challenge, message_type
        );
        log::info!("{}", challenge_info);
    }

    challenge.to_string()
}

// This function parses the event body received in the request
// and pulls out the Slack message text if there is any.
async fn intercept_command(body: &Value) {
    let event_type: &str = body["event"]["type"]
        .as_str()
        .unwrap_or("invalid event type");
    let username: &str = body["event"]["user"]["id"]
        .as_str()
        .unwrap_or("invalid_user_name");
    let first_name: &str = body["event"]["user"]["profile"]["first_name"]
        .as_str()
        .unwrap_or("");
    match event_type {
        "team_join" => commands::onboard_user::run(username, first_name).await,
        event_type => log::info!("invalid event type {}", event_type),
    }
    let channel: &str = body["event"]["channel"]
        .as_str()
        .unwrap_or("invalid_channel");
    let text: String = body["event"]["text"]
        .as_str()
        .unwrap_or("invalid_text")
        .to_lowercase();
    let enterprise_user_id: &str = body["enterprise_id"].as_str().unwrap_or("invalid_channel");
    let is_bot: bool = body["event"]["subtype"] == "bot_message";

    log::info!(
        "text: {}, channel: {}, user_id: {}, is_bot {}",
        text,
        channel,
        enterprise_user_id,
        is_bot
    );
    log::info!("Body {}", body);

    // Prevent responding to any channel other than Test
    if channel != "C0351GJ62Q0" {
        log::info!("This channel is not an allowed channel");
        return;
    }
    // Prevent responding to bots
    if is_bot {
        log::info!("This is a bot");
        return;
    }
    match text.as_str() {
        // Add new commands below and create new async functions for them.
        "ping" => commands::ping::run(channel).await,
        _ => log::info!("Invalid command: {:?}", ..),
    }
    if text.contains("buns") {
        commands::buns::run(channel, enterprise_user_id).await;
    }
}

// Helper function for getting Lambda environment variables. If
// you want to add new env vars, you can add them to the
// environment list in the devil-bot-rust-cdk-stack.ts file.
pub fn get_env_var(env_var: &str) -> String {
    match env::var(env_var) {
        Ok(val) => val,
        Err(_) => {
            log::info!("Required the {} environment variable", env_var);
            process::exit(1);
        }
    }
}

pub async fn increment_buns(enterprise_user_id: &str) {
    let buns_table_name: String = get_env_var(BUNS_TABLE_NAME);
    aws::dynamo::increment_item(&*buns_table_name, "user_id", enterprise_user_id, "buns")
        .await
        .unwrap_or_else(|err| log::info!("DynamoDB increment buns error: {}", err));
}
