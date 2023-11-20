use crate::openai::{self, ChatMessage};

pub(crate) fn summarize(
    raw_transcript: String,
    title: Option<String>,
    channel_name: Option<String>,
) -> Result<(Vec<openai::ChatMessage>, u64), String> {
    let words: usize = raw_transcript.split(' ').count();
    if words <= 200 {
        return Err(format!(
            "Transcript too short to summarize. ({words} words in transcript)"
        ));
    }
    let goal_length = (words / 5).min(2000);

    let messages = vec![
        openai::ChatMessage {
            role: "system",
            content: format!(
                "You are a summarization assistant. When the user gives you a message, you respond with a summary of the information inside. Just summarize the information without saying \"the speaker says\" or similar. The message will be an autogenerated transcript of a youtube video, and may have transcription errors and improperly separated speakers. Your summary should be about {goal_length} words.",
            ),
        },
        openai::ChatMessage {
            role: "user",
            content: format!(
                "{title}{channel}\n\nTranscript: {raw_transcript}\n\n\nBe as concise as possible in your summary. Repeat the information as without extra fluff like '{the_speaker} says'. Use full markdown syntax, and break the summary into paragraphs. Emphasize the most important information in **bold**. Remember that your summary should be about {goal_length} words. Just return the summary without repeating the Title or Channel, and don't write `Summary:`",
                title=title.map(|title| format!("Title: {title}")).unwrap_or_default(),
                channel=channel_name.clone().map(|channel_name| format!("\nChannel: {channel_name}")).unwrap_or_default(),
                the_speaker=channel_name.unwrap_or("the speaker".to_string()),
            ),
        },
    ];

    let chat_tokens = openai::count_tokens(&messages);

    Ok((messages, chat_tokens as u64))
}

fn clean_transcript_one_prompt(
    raw_transcript: String,
    title: Option<String>,
    channel_name: Option<String>,
) -> Vec<ChatMessage> {
    vec![
        openai::ChatMessage {
            role: "system",
            content:
                "You are a transcription assistant. The user will send an autogenerated transcript of a youtube video, which may have transcription errors, punctuation errors, and improperly separated speakers. You respond with a cleaned-up version of the transcript. The channel name and video title will be included in the message for additional context, but you should not include them in your response".to_string(),
        },
        openai::ChatMessage {
            role: "user",
            content: format!(
                "{title}{channel}\n\nTranscript: {raw_transcript}\n\n\nClean up the transcript above, fixing punctuation, transcription errors, and improperly separated speakers. Use full markdown syntax, and break it into paragraphs. Emphasize the most important information in **bold**. Just return the transcript without repeating the Title or Channel, and don't write `Transcript:`.",
                title=title.map(|title| format!("Title: {title}")).unwrap_or_default(),
                channel=channel_name.clone().map(|channel_name| format!("\nChannel: {channel_name}")).unwrap_or_default(),
            ),
        },
    ]
}

pub(crate) fn clean_transcript(
    raw_transcript: String,
    title: Option<String>,
    channel_name: Option<String>,
) -> Result<(Vec<Vec<openai::ChatMessage>>, u64), String> {
    let words = raw_transcript.split(' ').collect::<Vec<_>>();
    {
        let word_count = words.len();
        if word_count <= 20 {
            return Err(format!(
                "Transcript too short to clean up. ({word_count} words in transcript)"
            ));
        }
    }

    const MAX_TOKENS_PER_INVOCATION: usize = 50_000;

    let first_try =
        clean_transcript_one_prompt(raw_transcript.clone(), title.clone(), channel_name.clone());
    let chat_tokens = openai::count_tokens(&first_try);

    let chunks = {
        let chunks_necessary: usize = chat_tokens / MAX_TOKENS_PER_INVOCATION + 1;
        let chunk_size = words.len() / chunks_necessary + 1;
        words.chunks(chunk_size)
    };

    let prompts = chunks
        .map(|chunk| {
            let raw_transcript = chunk.join(" ");
            clean_transcript_one_prompt(raw_transcript, title.clone(), channel_name.clone())
        })
        .collect::<Vec<_>>();
    let total_tokens = prompts
        .iter()
        .map(|messages| openai::count_tokens(messages))
        .sum::<usize>();

    Ok((prompts, total_tokens as u64))
}
