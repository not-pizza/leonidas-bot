use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: &'static str,
    pub content: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ChatApiRequest {
    pub model: &'static str,
    pub messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
pub struct ChatApiResponse {
    pub choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessageContent,
}

#[derive(Deserialize)]
pub struct ChatMessageContent {
    pub content: String,
}

pub fn count_tokens(chat: &[ChatMessage]) -> usize {
    use tiktoken_rs::{get_chat_completion_max_tokens, ChatCompletionRequestMessage};
    let messages = chat
        .iter()
        .map(|message| ChatCompletionRequestMessage {
            content: Some(message.content.clone()),
            role: message.role.to_string(),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    get_chat_completion_max_tokens("gpt-4", &messages).unwrap()
}
