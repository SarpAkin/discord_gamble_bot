#![feature(backtrace)]

mod handler;
mod games;

use handler::*;
use serenity::{prelude::GatewayIntents, Client};
use std::fs;

async fn main_() -> RES {


    let token = match fs::read_to_string("token.txt"){
        Ok(t) => t,
        Err(_) => {assert!(false,"please provide a token at token.txt");"".to_string()},
    };

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILD_MEMBERS;

    let mut client = Client::builder(&token, intents).event_handler(Handler::new()?).await.expect("Err creating client");

    client.start().await?;


    Ok(())
}

#[tokio::main]
async fn main() -> RES {
    if let Err(err) = main_().await {
        eprintln!("Error: {:?} at {:?}", err, err.backtrace());
    }

    Ok(())
}
