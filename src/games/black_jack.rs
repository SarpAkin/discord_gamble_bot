use rand::prelude::*;
use scan_fmt::scan_fmt;
use serenity::{model::channel::Message, prelude::*};
use std::error::Error;
use std::time::Duration;
use std::vec;
use tokio::join;
use tokio::time::sleep;

use crate::handler::Handler;

#[derive(Copy, Clone)]
struct Card {
    id: u8,
}

impl Card {
    fn new(id: u32) -> Card {
        return Card { id: (id % 52) as u8 };
    }

    fn number(&self) -> u32 {
        return (self.id % 13) as u32;
    }

    fn create_deck() -> Vec<Card> {
        let mut deck: Vec<Card> = (0u32..(52u32 * 6)).map(|n| Card::new(n % 52)).collect();

        deck.shuffle(&mut rand::thread_rng());

        return deck;
    }
}

impl ToString for Card {
    fn to_string(&self) -> String {
        let number = self.id as u32 % 13;
        match self.id / 13 {
            0 => char::from_u32(0x1F0B1 + number),
            1 => char::from_u32(0x1F0C1 + number),
            2 => char::from_u32(0x1F0A1 + number),
            3 => char::from_u32(0x1F0D1 + number),
            _ => None,
        }
        .unwrap_or('0')
        .to_string()
    }
}

impl Handler {
    pub async fn black_jack(&self, ctx: Context, msg: Message) -> Result<(), Box<dyn Error + Sync + Send>> {
        #[rustfmt::skip]
        
        let gid = if let Some(gid) = msg.guild_id{gid} else {return Ok(());};

        let mut bet_amount = scan_fmt!(msg.content.as_str(), "!blackjack {}", i32)?;
        if bet_amount <= 0 {
            msg.channel_id.say(&ctx.http, format!("can't bet {}$", bet_amount)).await?;
            return Ok(());
        }

        let players_money = self.get_money(gid, msg.author.id).await?;
        if players_money < bet_amount {
            msg.channel_id.say(&ctx.http, format!("you only have {}$", players_money)).await?;
            return Ok(());
        }

        msg.channel_id.say(&ctx.http, format!("depositing {}$", bet_amount)).await?;

        self.inc_money(gid, msg.author.id, -bet_amount).await?;

        let mut deck = Card::create_deck();

        let mut players_cards = vec![deck.pop().unwrap(), deck.pop().unwrap()];
        let mut dealers_cards = vec![deck.pop().unwrap(), deck.pop().unwrap()];

        msg.channel_id
            .say(
                &ctx.http, //
                format!(
                    "dealers cards: XX {} score: {} \nplayers cards: {} {} score: {}\n\nhit(h), stand(s) or double(d)", //
                    black_jack_score(&vec![dealers_cards[1]]),
                    dealers_cards[1].to_string(),
                    players_cards[0].to_string(),
                    players_cards[1].to_string(),
                    black_jack_score(&players_cards)
                ),
            )
            .await?;

        let mut first_turn = true;

        loop {
            if let Some(reply) = msg.author.await_reply(ctx.shard.as_ref()).timeout(Duration::from_secs(10)).await {
                match reply.content.as_str().chars().nth(0).unwrap_or(' ') {
                    'h' | 'H' => {
                        players_cards.push(deck.pop().unwrap());
                        let player_score = black_jack_score(&players_cards);
                        let mut response = format!(
                            "players cards: {}\nscore: {}\n\nhit(h) or stand(s)", //
                            players_cards.iter().fold(String::new(), |acc, &card| acc + &card.to_string() + " "),
                            player_score
                        );
                        if player_score > 21 {
                            response += "\nbust!";
                            msg.channel_id.say(&ctx.http, response).await?;
                            break;
                        } else {
                            msg.channel_id.say(&ctx.http, response).await?;
                        }
                    }
                    's' | 'S' => {
                        break;
                    }
                    'd' | 'D' => {
                        if !first_turn {
                            msg.channel_id.say(&ctx.http, "can't double after first turn").await?;
                            break;
                        } else {
                            let players_money = self.get_money(gid, msg.author.id).await?;
                            if players_money < bet_amount {
                                msg.channel_id.say(&ctx.http, format!("you only have {}$\n continuing the game", players_money)).await?;
                            } else {
                                self.inc_money(gid, msg.author.id, -bet_amount).await?;
                                msg.channel_id.say(&ctx.http, format!("depositing another {}$", bet_amount)).await?;
                                bet_amount *= 2;
                                players_cards.push(deck.pop().unwrap());
                                let player_score = black_jack_score(&players_cards);
                                let mut response = format!(
                                    "players cards: {}\nscore: {}", //
                                    players_cards.iter().fold(String::new(), |acc, &card| acc + &card.to_string() + " "),
                                    player_score
                                );
                                if player_score > 21 {
                                    response += "\nbust!";
                                }
                                msg.channel_id.say(&ctx.http, response).await?;
                                break;
                            }
                        }
                    }
                    _ => {}
                }
                first_turn = false;
            }
        }

        msg.channel_id.say(&ctx.http, "dealers turn").await?;

        let mut d_message = msg
            .channel_id
            .say(
                &ctx.http,
                format!(
                    "dealers cards: {}\nscore: {}", //
                    dealers_cards.iter().fold(String::new(), |acc, &card| acc + &card.to_string() + " "),
                    black_jack_score(&dealers_cards)
                ),
            )
            .await?;

        while black_jack_score(&dealers_cards) < 17 {
            dealers_cards.push(deck.pop().unwrap());
            let dealers_score = black_jack_score(&dealers_cards);
            let mut content = format!(
                "dealers cards: {}\nscore: {}", //
                dealers_cards.iter().fold(String::new(), |acc, &card| acc + &card.to_string() + " "),
                dealers_score
            );
            if dealers_score > 21 {
                content += "\ndealer is busted";
                d_message.edit(ctx.http.as_ref(), |edit| edit.content(content)).await?;
                break;
            }

            let (a, _) = join![d_message.edit(ctx.http.as_ref(), |edit| edit.content(content)), sleep(Duration::from_secs_f32(1.2))];
            a?;
        }

        #[rustfmt::skip]
        let dealers_score = {let s = black_jack_score(&dealers_cards); if s > 21 {0} else {s} };
        #[rustfmt::skip]
        let players_score = {let s = black_jack_score(&players_cards); if s > 21 {0} else {s} };

        if players_score < dealers_score {
            msg.channel_id.say(&ctx.http, "dealer wins!").await?;
        } else {
            let reward = bet_amount * 2;

            msg.channel_id.say(&ctx.http, format!("player wins!\ngiving {}$", bet_amount * 2)).await?;
            self.inc_money(gid, msg.author.id, bet_amount * 2).await?;
        }

        return Ok(());
    }
}

fn black_jack_score(players_card: &Vec<Card>) -> u32 {
    let mut ace_count = 0u32;
    let mut score = 0u32;

    for card in players_card {
        score += match card.number() {
            0 => {
                ace_count += 1;
                1
            }
            10 => 10, // faces are 10 point
            11 => 10, // faces are 10 point
            12 => 10, // faces are 10 point
            a => a + 1,
        };
    }

    for _ in 0..ace_count {
        if score + 10 > 21 {
            break;
        } else {
            score += 10;
        }
    }

    return score;
}
