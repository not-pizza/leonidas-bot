#![feature(iter_intersperse)]

mod openai;
mod prompts;
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
            .filter_map(|url| youtube::video_id(url.as_str()))
            .collect();

        if !video_ids.is_empty() {
            // Sending a message can fail, due to a network error, an
            // authentication error, or lack of permissions to post in the
            // channel, so log to stdout when some error happens, with a
            // description of it.

            for video_id in video_ids {
                let typing = msg.channel_id.start_typing(&ctx.http);
                match youtube::get_video_summary(&video_id).await {
                    Ok((summary, info)) => {
                        let summary_chunks = break_text_into_chunks(summary, 4096);
                        let num_chunks = summary_chunks.len();
                        for (index, summary_chunk) in summary_chunks.into_iter().enumerate() {
                            let part = if num_chunks != 1 {
                                format!(" (part {}/{})", index + 1, num_chunks)
                            } else {
                                String::new()
                            };
                            if let Err(why) = msg
                                .channel_id
                                .send_message(&ctx.http, |m| {
                                    m.embed(|e| {
                                        e.title(format!("{}{part}", info.title.clone()))
                                            .description(summary_chunk)
                                            .footer(|f| f.text(info.channel_name.clone()))
                                    })
                                })
                                .await
                            {
                                println!("Error sending message: {:?}", why);
                            }
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
                let _ = typing.map(|typing| typing.stop());
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

fn break_text_into_chunks(s: String, max_characters_per_chunk: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    let paragraphs = s
        .split("\n")
        .map(|paragraph| paragraph.trim())
        .intersperse("\n\n")
        .flat_map(|paragraph| {
            if paragraph.chars().count() <= max_characters_per_chunk {
                vec![paragraph]
            } else {
                paragraph.split(" ").collect::<Vec<_>>()
            }
        })
        .collect::<Vec<_>>();

    for paragraph in paragraphs {
        // If we can't add the current paragraph to the current chunk, push the current chunk and start a new one
        if !current_chunk.is_empty()
            && current_chunk.chars().count() + paragraph.chars().count() > max_characters_per_chunk
        {
            chunks.push(current_chunk.trim().to_string());
            current_chunk = String::new();
        }

        // If we can add the current paragraph to the current chunk, do so
        current_chunk.push_str(&paragraph);
    }
    chunks.push(current_chunk);

    // Use regular expressions to find groups of newlines, and replace them all with 2 newlines
    chunks = {
        let re = regex::Regex::new(r"\n{3,}").unwrap();
        chunks
            .iter()
            .map(|chunk| re.replace_all(chunk, "\n\n").to_string())
            .collect()
    };

    chunks
}
