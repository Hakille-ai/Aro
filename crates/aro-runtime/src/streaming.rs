//! Real-time streaming action tracker for agent loops.
//!
//! Parses and extracts live stream deltas from model providers (Ollama, llama.cpp,
//! OpenAI-compatible, Anthropic, Gemini), immediately streaming:
//! 1. Thinking / reasoning blocks (`<think>...</think>`).
//! 2. Live content tokens without waiting for full JSON generation or blocking turns.
//! 3. Suppressing raw intermediate JSON framing while preserving tool-call metadata.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamDetectMode {
    Detecting,
    DirectText,
    JsonAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonFieldState {
    Scanning,
    InThought,
    InContent,
    Done,
}

#[derive(Default)]
pub struct StreamingActionTracker {
    in_think: bool,
    pub full_raw: String,
    streamed_content: String,
    streamed_any_content: bool,
    has_streamed_thinking: bool,
    mode: StreamDetectMode,
    post_think_buffer: String,
    json_state: JsonFieldState,
    json_parse_idx: usize,
    json_escape: bool,
}

impl Default for StreamDetectMode {
    fn default() -> Self {
        Self::Detecting
    }
}

impl Default for JsonFieldState {
    fn default() -> Self {
        Self::Scanning
    }
}

impl StreamingActionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    fn emit(on_chunk: &mut Option<&mut (dyn FnMut(String) + Send)>, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(cb) = on_chunk.as_deref_mut() {
            cb(text.to_string());
        }
    }

    pub fn push_chunk(
        &mut self,
        chunk: &str,
        on_chunk: &mut Option<&mut (dyn FnMut(String) + Send)>,
    ) {
        if chunk.is_empty() {
            return;
        }
        self.full_raw.push_str(chunk);

        let mut remainder = chunk;

        while !remainder.is_empty() {
            if self.in_think {
                if let Some(end_idx) = remainder.find("</think>") {
                    let thought_part = &remainder[..end_idx];
                    Self::emit(on_chunk, thought_part);
                    Self::emit(on_chunk, "</think>\n");
                    self.in_think = false;
                    remainder = &remainder[end_idx + "</think>".len()..];
                    if remainder.starts_with('\n') {
                        remainder = &remainder[1..];
                    }
                } else {
                    Self::emit(on_chunk, remainder);
                    return;
                }
            } else if let Some(start_idx) = remainder.find("<think>") {
                let before = &remainder[..start_idx];
                if !before.is_empty() {
                    self.process_post_think_slice(before, on_chunk);
                }
                Self::emit(on_chunk, "<think>");
                self.in_think = true;
                self.has_streamed_thinking = true;
                remainder = &remainder[start_idx + "<think>".len()..];
            } else {
                self.process_post_think_slice(remainder, on_chunk);
                return;
            }
        }
    }

    fn process_post_think_slice(
        &mut self,
        slice: &str,
        on_chunk: &mut Option<&mut (dyn FnMut(String) + Send)>,
    ) {
        self.post_think_buffer.push_str(slice);

        match self.mode {
            StreamDetectMode::Detecting => {
                let trimmed = self.post_think_buffer.trim_start();
                if trimmed.is_empty() {
                    return;
                }
                if trimmed.starts_with('{') || trimmed.starts_with("```") || trimmed.starts_with('`') {
                    self.mode = StreamDetectMode::JsonAction;
                    self.process_json_stream(on_chunk);
                } else if trimmed.len() >= 4 || trimmed.contains('\n') || trimmed.contains(' ') {
                    self.mode = StreamDetectMode::DirectText;
                    let to_emit = self.post_think_buffer.clone();
                    self.post_think_buffer.clear();
                    self.streamed_content.push_str(&to_emit);
                    self.streamed_any_content = true;
                    Self::emit(on_chunk, &to_emit);
                }
            }
            StreamDetectMode::DirectText => {
                let to_emit = self.post_think_buffer.clone();
                self.post_think_buffer.clear();
                self.streamed_content.push_str(&to_emit);
                self.streamed_any_content = true;
                Self::emit(on_chunk, &to_emit);
            }
            StreamDetectMode::JsonAction => {
                self.process_json_stream(on_chunk);
            }
        }
    }

    fn process_json_stream(
        &mut self,
        on_chunk: &mut Option<&mut (dyn FnMut(String) + Send)>,
    ) {
        loop {
            match self.json_state {
                JsonFieldState::Done => break,
                JsonFieldState::Scanning => {
                    if !self.has_streamed_thinking {
                        if let Some(pos) = find_json_field_value_start(&self.post_think_buffer, &["\"thought\"", "\"thinking\"", "\"reasoning\""]) {
                            Self::emit(on_chunk, "<think>");
                            self.in_think = true;
                            self.has_streamed_thinking = true;
                            self.json_state = JsonFieldState::InThought;
                            self.json_parse_idx = pos;
                            continue;
                        }
                    }

                    let content_keys = ["\"content\"", "\"response\"", "\"text\"", "\"message\"", "\"answer\""];
                    if let Some(pos) = find_json_field_value_start(&self.post_think_buffer, &content_keys) {
                        self.json_state = JsonFieldState::InContent;
                        self.json_parse_idx = pos;
                        continue;
                    }

                    break;
                }
                JsonFieldState::InThought => {
                    let mut slice = String::new();
                    let mut quote_closed = false;

                    let unparsed = &self.post_think_buffer[self.json_parse_idx..];
                    for c in unparsed.chars() {
                        self.json_parse_idx += c.len_utf8();

                        if self.json_escape {
                            self.json_escape = false;
                            match c {
                                'n' => slice.push('\n'),
                                'r' => slice.push('\r'),
                                't' => slice.push('\t'),
                                '"' => slice.push('"'),
                                '\\' => slice.push('\\'),
                                other => {
                                    slice.push('\\');
                                    slice.push(other);
                                }
                            }
                        } else if c == '\\' {
                            self.json_escape = true;
                        } else if c == '"' {
                            quote_closed = true;
                            break;
                        } else {
                            slice.push(c);
                        }
                    }

                    if !slice.is_empty() {
                        Self::emit(on_chunk, &slice);
                    }

                    if quote_closed {
                        Self::emit(on_chunk, "</think>\n");
                        self.in_think = false;
                        self.json_state = JsonFieldState::Scanning;
                    } else {
                        break;
                    }
                }
                JsonFieldState::InContent => {
                    let mut slice = String::new();
                    let mut quote_closed = false;

                    let unparsed = &self.post_think_buffer[self.json_parse_idx..];
                    for c in unparsed.chars() {
                        self.json_parse_idx += c.len_utf8();

                        if self.json_escape {
                            self.json_escape = false;
                            match c {
                                'n' => slice.push('\n'),
                                'r' => slice.push('\r'),
                                't' => slice.push('\t'),
                                '"' => slice.push('"'),
                                '\\' => slice.push('\\'),
                                other => {
                                    slice.push('\\');
                                    slice.push(other);
                                }
                            }
                        } else if c == '\\' {
                            self.json_escape = true;
                        } else if c == '"' {
                            quote_closed = true;
                            break;
                        } else {
                            slice.push(c);
                        }
                    }

                    if !slice.is_empty() {
                        self.streamed_content.push_str(&slice);
                        self.streamed_any_content = true;
                        Self::emit(on_chunk, &slice);
                    }

                    if quote_closed {
                        self.json_state = JsonFieldState::Done;
                    }
                    break;
                }
            }
        }
    }

    pub fn finalize(
        &mut self,
        on_chunk: &mut Option<&mut (dyn FnMut(String) + Send)>,
    ) {
        if self.in_think {
            Self::emit(on_chunk, "</think>\n");
            self.in_think = false;
        }
        if self.mode == StreamDetectMode::Detecting && !self.post_think_buffer.trim().is_empty() {
            let trimmed = self.post_think_buffer.trim_start();
            if !trimmed.starts_with('{') {
                let text = self.post_think_buffer.clone();
                self.streamed_content.push_str(&text);
                self.streamed_any_content = true;
                Self::emit(on_chunk, &text);
            }
        }
    }

    pub fn streamed_content(&self) -> &str {
        &self.streamed_content
    }

    pub fn has_streamed_content(&self) -> bool {
        self.streamed_any_content && !self.streamed_content.trim().is_empty()
    }
}

fn find_json_field_value_start(buffer: &str, keys: &[&str]) -> Option<usize> {
    for key in keys {
        if let Some(key_idx) = buffer.find(key) {
            let after_key = &buffer[key_idx + key.len()..];
            let mut chars = after_key.char_indices();
            let mut colon_seen = false;
            while let Some((c_idx, c)) = chars.next() {
                if !colon_seen {
                    if c == ':' {
                        colon_seen = true;
                    } else if !c.is_whitespace() {
                        break;
                    }
                } else {
                    if c.is_whitespace() {
                        continue;
                    } else if c == '"' {
                        return Some(key_idx + key.len() + c_idx + 1);
                    } else {
                        break;
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_direct_prose() {
        let mut chunks = Vec::new();
        let mut cb = |c: String| chunks.push(c);
        let mut on_chunk: Option<&mut (dyn FnMut(String) + Send)> = Some(&mut cb);
        let mut tracker = StreamingActionTracker::new();

        tracker.push_chunk("Hello ", &mut on_chunk);
        tracker.push_chunk("world, ", &mut on_chunk);
        tracker.push_chunk("how are you?", &mut on_chunk);
        tracker.finalize(&mut on_chunk);

        assert_eq!(chunks.concat(), "Hello world, how are you?");
        assert!(tracker.has_streamed_content());
        assert_eq!(tracker.streamed_content(), "Hello world, how are you?");
    }

    #[test]
    fn test_streaming_with_think_tags() {
        let mut chunks = Vec::new();
        let mut cb = |c: String| chunks.push(c);
        let mut on_chunk: Option<&mut (dyn FnMut(String) + Send)> = Some(&mut cb);
        let mut tracker = StreamingActionTracker::new();

        tracker.push_chunk("<think>", &mut on_chunk);
        tracker.push_chunk("Thinking ", &mut on_chunk);
        tracker.push_chunk("step by step", &mut on_chunk);
        tracker.push_chunk("</think>\n", &mut on_chunk);
        tracker.push_chunk("{\"action\":\"final\",\"content\":\"The answer is 42\"}", &mut on_chunk);
        tracker.finalize(&mut on_chunk);

        let full_emitted = chunks.concat();
        assert!(full_emitted.starts_with("<think>Thinking step by step</think>\n"));
        assert!(full_emitted.ends_with("The answer is 42"));
        assert_eq!(tracker.streamed_content(), "The answer is 42");
    }

    #[test]
    fn test_streaming_json_content_extraction() {
        let mut chunks = Vec::new();
        let mut cb = |c: String| chunks.push(c);
        let mut on_chunk: Option<&mut (dyn FnMut(String) + Send)> = Some(&mut cb);
        let mut tracker = StreamingActionTracker::new();

        tracker.push_chunk("{\"action\": ", &mut on_chunk);
        tracker.push_chunk("\"final\", \"content\": ", &mut on_chunk);
        tracker.push_chunk("\"Bonjour ", &mut on_chunk);
        tracker.push_chunk("le monde !\\n", &mut on_chunk);
        tracker.push_chunk("Tout va bien.\"", &mut on_chunk);
        tracker.push_chunk("}", &mut on_chunk);
        tracker.finalize(&mut on_chunk);

        let emitted = chunks.concat();
        assert_eq!(emitted, "Bonjour le monde !\nTout va bien.");
        assert_eq!(tracker.streamed_content(), "Bonjour le monde !\nTout va bien.");
        assert!(tracker.has_streamed_content());
    }

    #[test]
    fn test_streaming_tool_call_suppression() {
        let mut chunks = Vec::new();
        let mut cb = |c: String| chunks.push(c);
        let mut on_chunk: Option<&mut (dyn FnMut(String) + Send)> = Some(&mut cb);
        let mut tracker = StreamingActionTracker::new();

        tracker.push_chunk("{\"action\": \"tool\", \"tool_id\": \"core.search.web\", \"input\": {\"query\": \"paris weather\"}}", &mut on_chunk);
        tracker.finalize(&mut on_chunk);

        assert_eq!(chunks.concat(), "");
        assert!(!tracker.has_streamed_content());
    }

    #[test]
    fn test_streaming_json_thought_then_content() {
        let mut chunks = Vec::new();
        let mut cb = |c: String| chunks.push(c);
        let mut on_chunk: Option<&mut (dyn FnMut(String) + Send)> = Some(&mut cb);
        let mut tracker = StreamingActionTracker::new();

        tracker.push_chunk("{\"thought\": \"Réfléchissons un instant\", ", &mut on_chunk);
        tracker.push_chunk("\"action\": \"final\", ", &mut on_chunk);
        tracker.push_chunk("\"content\": \"Voici la réponse.\"}", &mut on_chunk);
        tracker.finalize(&mut on_chunk);

        let emitted = chunks.concat();
        assert_eq!(emitted, "<think>Réfléchissons un instant</think>\nVoici la réponse.");
        assert_eq!(tracker.streamed_content(), "Voici la réponse.");
    }
}
