pub mod black_jack;

use rand::prelude::*;
use scan_fmt::scan_fmt;
use serenity::{model::channel::Message, prelude::*};
use std::error::Error;
use std::time::Duration;
use std::vec;
use tokio::join;
use tokio::time::sleep;

use crate::handler::Handler;


//slot \u1F3B0
//cherry \u1F352
//watermelon \u1F349
//diamond \u1F48E

enum SlotSymbols {
    Cherry,

}

impl Handler {
    async fn slot(&self, ctx: Context, msg: Message) -> Result<(), Box<dyn Error + Sync + Send>> {
        

        Ok(())
    }
}
