#![feature(iter_intersperse)]

mod openai;
mod prompts;
mod utils;
mod youtube;

use std::env;

use dotenv::dotenv;
use linkify::{LinkFinder, LinkKind};
use serenity::all::{ChannelId, ReactionType};
use serenity::async_trait;
use serenity::builder::{CreateEmbed, CreateEmbedFooter, CreateMessage};
use serenity::model::channel::{Message, Reaction};
use serenity::model::gateway::Ready;
use serenity::prelude::*;

struct Handler;

const TRANSCRIBE_EMOJI: &str = "📜";
const SUMMARIZE_EMOJI: &str = "💭";

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // make sure the message isn't from a bot
        if msg.author.bot {
            return;
        }

        let video_ids = video_ids_for_message(&msg);

        if !video_ids.is_empty() {
            // Sending a message can fail, due to a network error, an
            // authentication error, or lack of permissions to post in the
            // channel, so log to stdout when some error happens, with a
            // description of it.

            msg.react(
                &ctx.http,
                ReactionType::Unicode(SUMMARIZE_EMOJI.to_string()),
            )
            .await
            .unwrap();

            msg.react(
                &ctx.http,
                ReactionType::Unicode(TRANSCRIBE_EMOJI.to_string()),
            )
            .await
            .unwrap();
        }
    }

    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        enum Action {
            Transcribe,
            Summarize,
        }
        if reaction
            .member
            .as_ref()
            .map(|user| user.user.bot)
            .unwrap_or(true)
        {
            return;
        }
        if reaction.emoji.unicode_eq(TRANSCRIBE_EMOJI) {
            if let Ok(message) = reaction.message(&ctx.http).await {
                transcribe_videos(ctx, &message).await;
            }
        } else if reaction.emoji.unicode_eq(SUMMARIZE_EMOJI) {
            if let Ok(message) = reaction.message(&ctx.http).await {
                summarize_videos(ctx, &message).await;
            }
        };
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

fn video_ids_for_message(msg: &Message) -> Vec<String> {
    LinkFinder::new()
        .links(&msg.content)
        .filter(|link| link.kind() == &LinkKind::Url)
        // get the ids of youtube videos linked in the message
        .filter_map(|url| youtube::video_id(url.as_str()))
        .collect()
}

async fn send_video_description(
    ctx: &Context,
    content: String,
    info: youtube::VideoInfo,
    channel_id: ChannelId,
) {
    let summary_chunks = utils::break_text_into_chunks(content, 4096);
    let num_chunks = summary_chunks.len();
    for (index, summary_chunk) in summary_chunks.into_iter().enumerate() {
        let part = if num_chunks != 1 {
            format!(" (part {}/{})", index + 1, num_chunks)
        } else {
            String::new()
        };

        let embed = CreateEmbed::new()
            .title(format!("{}{part}", info.title.clone()))
            .description(summary_chunk)
            .footer(CreateEmbedFooter::new(info.channel_name.clone()));
        let message = CreateMessage::new().embed(embed);
        if let Err(why) = channel_id.send_message(&ctx.http, message).await {
            println!("Error sending message: {:?}", why);
        }
    }
}

async fn summarize_videos(ctx: Context, msg: &Message) {
    let video_ids = video_ids_for_message(msg);
    for video_id in video_ids {
        let typing = msg.channel_id.start_typing(&ctx.http);
        match youtube::get_video_summary(&video_id).await {
            Ok((summary, info)) => {
                send_video_description(&ctx, summary, info, msg.channel_id).await;
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
        let _ = typing.stop();
    }
}

async fn transcribe_videos(ctx: Context, msg: &Message) {
    let video_ids = video_ids_for_message(msg);
    for video_id in video_ids {
        let typing = msg.channel_id.start_typing(&ctx.http);
        match youtube::get_video_transcript(&video_id).await {
            Ok((summary, info)) => {
                send_video_description(&ctx, summary, info, msg.channel_id).await;
            }
            Err(why) => {
                if let Err(why) = msg
                    .channel_id
                    .say(&ctx.http, format!("Transcription error: {why:?}"))
                    .await
                {
                    println!("Error sending message: {:?}", why);
                }
            }
        }
        let _ = typing.stop();
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Configure the client with your Discord bot token in the environment.
    let token = discord_token().expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGE_REACTIONS;

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
