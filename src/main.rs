mod youtube;

use std::env;

use dotenv::dotenv;
use linkify::{LinkFinder, LinkKind};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event - so that whenever a new message
    // is received - the closure (or function) passed will be called.
    //
    // Event handlers are dispatched through a threadpool, and so multiple
    // events can be dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        // make sure the message isn't from a bot
        if msg.author.bot {
            return;
        }

        let video_ids: Vec<_> = LinkFinder::new()
            .links(&msg.content)
            .filter(|link| link.kind() == &LinkKind::Url)
            // get the ids of youtube videos linked in the message
            .filter_map(|url| {
                regex::Regex::new(r"(?:https://www\.youtube\.com/watch\?v=|https://youtu\.be/)(?P<id>[a-zA-Z0-9_-]+).*")
                    .unwrap()
                    .captures(url.as_str())
                    .and_then(|captures| captures.name("id"))
                    .map(|id| id.as_str())
            })
            .collect();

        if !video_ids.is_empty() {
            // Sending a message can fail, due to a network error, an
            // authentication error, or lack of permissions to post in the
            // channel, so log to stdout when some error happens, with a
            // description of it.

            for video_id in video_ids {
                match youtube::get_video_summary(video_id).await {
                    Ok(summary) => {
                        if let Err(why) = msg
                            .channel_id
                            .send_message(&ctx.http, |m| {
                                m.embed(|e| e.title("TL;DW").description(summary))
                            })
                            .await
                        {
                            println!("Error sending message: {:?}", why);
                        }
                    }
                    Err(why) => {
                        if let Err(why) = msg
                            .channel_id
                            .say(&ctx.http, format!("Summary error: {why:?}"))
                            .await
                        {
                            println!("Error sending message: {:?}", why);
                        }
                    }
                }
            }
        }
    }

    // Set a handler to be called on the `ready` event. This is called when a
    // shard is booted, and a READY payload is sent by Discord. This payload
    // contains data like the current user's guild Ids, current user data,
    // private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

fn discord_token() -> Option<String> {
    for (key, value) in env::vars() {
        if key == "DISCORD_TOKEN" {
            return Some(value);
        }
    }
    None
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Configure the client with your Discord bot token in the environment.
    let token = discord_token().expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
