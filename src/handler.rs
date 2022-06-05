use rand::prelude::*;
use scan_fmt::scan_fmt;
use serenity::futures::future::join_all;
use serenity::model::guild::Member;
use serenity::model::id::{GuildId, UserId};
use serenity::model::mention;
use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready, guild::Guild, prelude::User},
    prelude::*,
    utils::MessageBuilder,
};

use serde::{Deserialize, Serialize};

use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write;
use std::time;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type RES = Result<(), Box<dyn Error + Sync + Send>>;

// mod crate::black_jack;

pub struct Handler {
    db: Mutex<Connection>,
}

impl Handler {
    pub fn new() -> Result<Handler, Box<dyn Error + Sync + Send>> {
        let db = Connection::open("discord_bot.db")?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS guilds (
                GUILD_ID    INT8    KEY NOT NULL UNIQUE,
                NAME        CHAR(32)    NOT NULL
            );",
            [],
        )
        .unwrap();

        Ok(Handler { db: Mutex::new(db) })
    }

    pub async fn inc_money(&self, gid: GuildId, pid: UserId, amount: i32) -> RES {
        let db = self.db.lock().await;

        db.execute(format!("UPDATE guild_{} SET MONEY = MONEY + {} WHERE USER_ID = {};", gid.0, amount, pid.0).as_str(), []).unwrap();

        Ok(())
    }
    //ALTER TABLE {tableName} ADD COLUMN COLNew {type};
    pub async fn get_money(&self, gid: GuildId, pid: UserId) -> Result<i32, Box<dyn Error + Sync + Send>> {
        let db = self.db.lock().await;

        let mut stmt = db.prepare(format!("SELECT MONEY FROM guild_{} WHERE USER_ID = {};", gid.0, pid.0).as_str()).unwrap();

        let a = stmt.query_map([], |row| Ok(row.get(0)?))?.into_iter().collect::<Result<Vec<i32>, _>>()?;

        Ok(a[0])
    }

    async fn balance(&self, ctx: Context, msg: Message) -> RES {
        println!("balance called msg contents:\n {}", msg.content);

        if msg.mentions.len() == 0 {
            return Ok(());
        }

        let response = {
            if let Some(gid) = msg.guild_id {
                let mut rep = String::new();

                for m in msg.mentions {
                    write!(&mut rep, "player {} has {}$", m.name, self.get_money(gid, m.id).await?)?;
                }

                rep
            } else {
                String::new()
            }
        };

        msg.channel_id.say(&ctx.http, response).await?;

        Result::Ok(())
    }

    async fn balltop(&self, ctx: Context, msg: Message) -> RES {
        #[rustfmt::skip]
        let gid = if let Some(gid) = msg.guild_id{gid} else {return Ok(());};

        let res = {
            let db = self.db.lock().await;

            struct Player(String, i32, u64);

            let mut res = String::new();

            let mut stmt = db.prepare(format!("SELECT NICK,MONEY,USER_ID FROM guild_{} ORDER BY MONEY DESC LIMIT 10;", gid).as_str()).unwrap();
            for r in stmt.query_map([], |row| Ok(Player(row.get(0)?, row.get(1)?, row.get(2)?)))?.into_iter() {
                let Player(name, money, _id) = r?;
                write!(&mut res, "{}: {}$\n", name, money)?;
            }
            res
        };

        msg.channel_id.say(&ctx.http, res).await?;

        Ok(())
    }

    async fn claim_daily(&self, ctx: Context, msg: Message) -> RES {

        let gid = if let Some(gid) = msg.guild_id {
            gid
        } else {
            return Ok(());
        };

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let (rep, amount) = {
            let db = self.db.lock().await;

            let res = if let Some(date_u64) = db //
                .prepare(format!("SELECT LAST_DAILY FROM guild_{} WHERE USER_ID = {}", gid, msg.author.id).as_str())
                .unwrap()
                .query_map([], |row| Ok(row.get(0)?))?
                .into_iter()
                .collect::<Result<Vec<u64>, _>>()?
                .get(0)
            {
                if *date_u64 + 3600 * 24 < (now) {
                    let daily_amount = 3000;
                    db.execute(format!("UPDATE guild_{} SET LAST_DAILY = {} WHERE USER_ID = {};", gid.0, now, msg.author.id.0).as_str(), []).unwrap();

                    (format!("claiming daily {}$", daily_amount), daily_amount)
                } else {
                    (format!("claimed daily in last 24hours\nwait for {} seconds to claim again", (*date_u64 + 3600 * 24) - now), 0)
                }
            } else {
                return Ok(());
            };

            res
        };

        if amount > 0 {
            self.inc_money(gid, msg.author.id, amount).await?;
        }

        msg.channel_id.say(&ctx.http, rep).await?;

        Ok(())
    }

    async fn game(&self, ctx: Context, msg: Message) -> RES {
        msg.channel_id.say(&ctx.http, "guess the number between 1 and 100").await?;

        let max_attemps = 10;
        let number = (rand::thread_rng().next_u32() % 100) as i32;
        for i in 0..max_attemps {
            if let Some(reply) = msg.author.await_reply(ctx.shard.as_ref()).timeout(Duration::from_secs(10)).await {
                if let Ok(guess) = reply.content.parse::<i32>() {
                    match guess {
                        _ if guess < number => {
                            msg.channel_id.say(&ctx.http, "your guess is too small").await?;
                        }

                        _ if guess > number => {
                            msg.channel_id.say(&ctx.http, "your guess is too big").await?;
                        }
                        _ if guess == number => {
                            msg.channel_id.say(&ctx.http, format!("your guessed the number in {} tries", i)).await?;

                            return Ok(());
                        }

                        _ => {}
                    }
                } else {
                    msg.channel_id.say(&ctx.http, "not a number").await?;
                }
            } else {
                return Ok(());
            }
        }
        msg.channel_id.say(&ctx.http, format!("your have failed to guess the number after {} attempts. the number was {}!", max_attemps, number)).await?;

        return Ok(());
    }

    async fn give_money(&self, ctx: Context, msg: Message) -> RES {
        println!("{}", &msg.content);

        let amount = scan_fmt!(&msg.content, "!give_money {}", i32)?;

        if let Some(gid) = msg.guild_id {
            self.inc_money(gid, msg.author.id, amount).await?;
        }

        Result::Ok(())
    }

    async fn handle_message(&self, ctx: Context, msg: Message) -> RES {
        match *msg.content.split(' ').collect::<Vec<&str>>().get(0).unwrap_or(&"-") {
            "!ping" => {
                let response = MessageBuilder::new().push("!pong ").mention(&msg.author).build();
                msg.channel_id.say(&ctx.http, response).await?;
            }
            "!game" => self.game(ctx, msg).await?,
            "!blackjack" => self.black_jack(ctx, msg).await?,
            "!balance" => self.balance(ctx, msg).await?,
            "!give_money" => self.give_money(ctx, msg).await?,
            "!balltop" => self.balltop(ctx, msg).await?,
            "!claim_daily" => self.claim_daily(ctx, msg).await?,
            "!?" => {
                msg.channel_id.say(&ctx.http, "!blackjack\n!balltop\n!balance\n!claim_daily").await?;
            }
            _ => {}
        }

        return Ok(());
    }

    async fn ready_(&self, ctx: Context, ready: Ready) -> RES {
        println!("{} is connected!", ready.user.name);

        let _ = join_all(ready.guilds.into_iter().map(|g| self.register_guild(&ctx, g.id))).await.into_iter().collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}

impl Handler {
    async fn register_guild(&self, ctx: &Context, gid: GuildId) -> RES {
        let partial_g = gid.to_partial_guild(&ctx.http).await?;
        let members = gid.members(&ctx.http, None, None).await?;

        let db = self.db.lock().await;
        db.execute(
            format!(
                "CREATE TABLE IF NOT EXISTS guild_{} (
                USER_ID     INT8    KEY NOT NULL,
                MONEY       INT         NOT NULL,
                NICK        CHAR(32)    NOT NULL,
                LAST_DAILY  INT8        NOT NULL,
                UNIQUE(USER_ID)
            );",
                { gid.0 }
            )
            .as_str(),
            params![],
        )
        .unwrap();

        db.execute("INSERT OR REPLACE INTO guilds (GUILD_ID,NAME) VALUES (?1 ,?2);", params![gid.0, partial_g.name]).unwrap();

        let sql = format!("INSERT OR IGNORE INTO guild_{} (USER_ID,MONEY,NICK,LAST_DAILY) VALUES (?1,?2,?3,0);", gid.0);
        for m in members {
            let name = m.nick.unwrap_or(m.user.name);
            db.execute(sql.as_str(), params![m.user.id.0, 0, name]).unwrap();
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if let Err(err) = self.handle_message(ctx, msg).await {
            eprintln!("Error: {:?} at {:?}", err, err.backtrace());
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        if let Err(err) = self.ready_(ctx, ready).await {
            eprintln!("Error: {:?} at {:?}", err, err.backtrace());
        }
    }

    async fn guild_member_addition(&self, ctx: Context, new_member: Member) {
        let sql = format!("INSERT OR REPLACE INTO guild_{} (USER_ID,MONEY,NICK) VALUES (?1,?2,?3);", new_member.guild_id.0);

        let db = self.db.lock().await;

        db.execute(sql.as_str(), params![new_member.user.id.0, 0, new_member.nick.unwrap_or(new_member.user.name)]).unwrap();
    }
}
