use std::{env, time::Duration};

use serde::{Deserialize, Serialize};

use crate::openai;
use crate::prompts;

fn youtube_token() -> Option<String> {
    for (key, value) in env::vars() {
        if key == "YOUTUBE_API_TOKEN" {
            return Some(value);
        }
    }
    None
}

fn openai_token() -> Option<String> {
    for (key, value) in env::vars() {
        if key == "OPENAI_API_TOKEN" {
            return Some(value);
        }
    }
    None
}

pub fn video_id(url: &str) -> Option<String> {
    regex::Regex::new(r"(?:https://www\.youtube\.com/watch\?v=|https://youtu\.be/|https://www.youtube.com/shorts/)(?P<id>[a-zA-Z0-9_-]+).*")
                    .unwrap()
                    .captures(url)
                    .and_then(|captures| captures.name("id"))
                    .map(|id| id.as_str().to_string())
}

#[derive(Serialize, Deserialize)]
struct TranscriptItem {
    text: String,
    #[allow(dead_code)]
    start: f64,
    #[allow(dead_code)]
    duration: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum TranscriptResponse {
    Success { transcript: Vec<TranscriptItem> },
    Error { message: String },
}

#[derive(Serialize, Deserialize)]
struct Snippet {
    title: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
}

#[derive(Serialize, Deserialize)]
struct Item {
    snippet: Snippet,
}

#[derive(Serialize, Deserialize)]
struct VideoResponse {
    items: Vec<Item>,
}

#[derive(Debug)]
pub struct VideoInfo {
    pub title: String,
    pub channel_name: String,
}

async fn get_transcript(video_id: &str) -> Result<String, String> {
    let url = format!(
        "https://zl319yz4a6.execute-api.us-east-1.amazonaws.com/Prod/youtube/transcript/{}",
        video_id
    );
    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    let data: TranscriptResponse = response.json().await.map_err(|e| e.to_string())?;

    match data {
        TranscriptResponse::Success { transcript } => Ok(transcript
            .iter()
            .map(|item| item.text.clone())
            .collect::<Vec<String>>()
            .join(" ")),
        TranscriptResponse::Error { message } => {
            eprintln!("Error fetching transcript: {}", message);
            Err(message)
        }
    }
}

async fn get_video_info(video_id: &str) -> Result<VideoInfo, reqwest::Error> {
    let url = format!(
        "https://www.googleapis.com/youtube/v3/videos?id={}&key={}&part=snippet",
        video_id,
        youtube_token().unwrap()
    );
    let response = reqwest::get(&url).await?;
    let video_response: VideoResponse = response.json().await?;
    let item = &video_response.items[0];
    Ok(VideoInfo {
        title: item.snippet.title.clone(),
        channel_name: item.snippet.channel_title.clone(),
    })
}

async fn chat(chat_api_request: openai::ChatApiRequest) -> Result<String, String> {
    async fn chat_once(chat_api_request: openai::ChatApiRequest) -> Result<String, String> {
        let client = reqwest::Client::new();
        let api_response = client
            .post("https://zl319yz4a6.execute-api.us-east-1.amazonaws.com/Prod/v1/chat/completions")
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", openai_token().unwrap()),
            )
            .json(&chat_api_request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let data: openai::ChatApiResponse = api_response.json().await.map_err(|e| e.to_string())?;

        if let Some(first_choice) = data.choices.get(0) {
            Ok(first_choice.message.content.clone())
        } else {
            Err("No choices in response".to_string())
        }
    }
    match chat_once(chat_api_request.clone()).await {
        Ok(response) => Ok(response),
        Err(_e) => {
            // Try again in one minute
            tokio::time::sleep(Duration::from_secs(60)).await;
            chat_once(chat_api_request).await
        }
    }
}

async fn summarize(
    raw_transcript: String,
    title: Option<String>,
    channel_name: Option<String>,
) -> Result<String, String> {
    let messages = prompts::summarize(raw_transcript, title, channel_name)?;

    let chat_tokens = openai::count_tokens(messages.clone());
    let model = if chat_tokens > 13000 {
        return Err(format!(
            "Transcript too long to summarize. ({} tokens)",
            chat_tokens
        ));
    } else if chat_tokens < 3000 {
        "gpt-4"
    } else {
        "gpt-3.5-turbo-16k"
    };

    let chat_api_request = openai::ChatApiRequest { model, messages };

    chat(chat_api_request).await
}

pub async fn get_video_summary(video_id: &str) -> Result<(String, VideoInfo), String> {
    let info = get_video_info(video_id).await.map_err(|e| e.to_string())?;
    let transcript = get_transcript(video_id).await?;
    let summary = summarize(
        transcript,
        Some(info.title.clone()),
        Some(info.channel_name.clone()),
    )
    .await?;
    let clipped_summary = if summary.len() > 4096 {
        format!("{}...", summary.chars().take(4093).collect::<String>())
    } else {
        summary
    };
    Ok((clipped_summary, info))
}
