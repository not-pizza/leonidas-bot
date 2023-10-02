pub fn break_text_into_chunks(s: String, max_characters_per_chunk: usize) -> Vec<String> {
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
