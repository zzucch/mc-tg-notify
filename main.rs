use config::{Config, File};
use reqwest::Error;
use serde::Deserialize;

#[derive(Deserialize)]
struct StatusResponse {
    online: bool,
    players: Option<Players>,
}

#[derive(Deserialize)]
struct Players {
    online: i32,
    sample: Option<Vec<Player>>,
}

#[derive(Deserialize)]
struct Player {
    name: String,
}

#[derive(Deserialize)]
struct TelegramConfig {
    bot_token: String,
    chat_id: i64,
}

#[derive(Deserialize)]
struct ServerConfig {
    ip: String,
}

async fn get_server_status(ip: &str) -> Result<StatusResponse, Error> {
    let url = format!("https://api.mcsrvstat.us/2/{}", ip);
    let response = reqwest::get(&url).await?.json().await?;
    Ok(response)
}

async fn send_telegram_message(bot_token: &str, chat_id: i64, text: &str) -> Result<(), Error> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let params = [("chat_id", chat_id.to_string()), ("text", text.to_string())];
    reqwest::Client::new()
        .post(&url)
        .form(&params)
        .send()
        .await?;
    Ok(())
}

async fn handle_server_status(
    status: StatusResponse,
    last_result: &mut i32,
    telegram_config: &TelegramConfig,
) -> Result<(), Error> {
    if !status.online {
        println!("Server is down.");
        return Ok(());
    }

    let players = match status.players {
        Some(players) => players,
        None => panic!("Players field must be provided when server is up"),
    };

    let players_online = players.online;
    let player_names: Vec<String> = players
        .sample
        .unwrap_or_default()
        .into_iter()
        .map(|player| player.name)
        .collect();

    println!("Players online: {}", players_online);
    if !player_names.is_empty() {
        println!("Player names: {:?}", player_names);
    }

    if players_online == *last_result {
        return Ok(());
    }

    if *last_result != -1 && players_online > 0 {
        let message = format!("{}", players_online);

        if let Err(e) = send_telegram_message(
            &telegram_config.bot_token,
            telegram_config.chat_id,
            &message,
        )
        .await
        {
            return Err(e.into());
        }
    }

    *last_result = players_online;

    Ok(())
}

async fn monitor_server_status(server_config: &ServerConfig, telegram_config: &TelegramConfig) {
    let mut last_result = -1;

    loop {
        match get_server_status(&server_config.ip).await {
            Ok(status) => {
                if let Err(e) =
                    handle_server_status(status, &mut last_result, &telegram_config).await
                {
                    eprintln!("{}", e);
                }
            }
            Err(e) => {
                eprintln!("{}", e);
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

fn main() {
    let mut settings = Config::default();

    settings.merge(File::with_name("config")).unwrap();

    let server_config = settings.get::<ServerConfig>("server").unwrap();
    let telegram_config = settings.get::<TelegramConfig>("telegram").unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(monitor_server_status(&server_config, &telegram_config));
}
