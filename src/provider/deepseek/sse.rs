#[derive(Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
}

impl SseDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, String> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(newline) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8(line)
                .map_err(|_| "DeepSeek returned a non-UTF-8 SSE line".to_owned())?;
            self.accept_line(&line, &mut events);
        }
        Ok(events)
    }

    pub fn finish(mut self) -> Result<Vec<String>, String> {
        let mut events = Vec::new();
        if !self.buffer.is_empty() {
            let line = String::from_utf8(std::mem::take(&mut self.buffer))
                .map_err(|_| "DeepSeek returned a non-UTF-8 SSE line".to_owned())?;
            self.accept_line(line.trim_end_matches('\r'), &mut events);
        }
        self.flush_event(&mut events);
        Ok(events)
    }

    fn accept_line(&mut self, line: &str, events: &mut Vec<String>) {
        if line.is_empty() {
            self.flush_event(events);
        } else if let Some(data) = line.strip_prefix("data:") {
            self.data_lines
                .push(data.strip_prefix(' ').unwrap_or(data).to_owned());
        }
    }

    fn flush_event(&mut self, events: &mut Vec<String>) {
        if !self.data_lines.is_empty() {
            events.push(self.data_lines.join("\n"));
            self.data_lines.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_split_crlf_and_multiple_events() {
        let mut decoder = SseDecoder::default();
        assert!(
            decoder
                .push(b"data: {\"choices\":[{\"delta\":{\"con")
                .unwrap()
                .is_empty()
        );
        let events = decoder
            .push(b"tent\":\"hi\"}}]}\r\n\r\ndata: [DONE]\n\n")
            .unwrap();

        assert_eq!(events.len(), 2);
        assert!(events[0].contains("\"content\":\"hi\""));
        assert_eq!(events[1], "[DONE]");
    }

    #[test]
    fn joins_multiline_data_fields() {
        let mut decoder = SseDecoder::default();
        let events = decoder.push(b"data: first\ndata: second\n\n").unwrap();
        assert_eq!(events, ["first\nsecond"]);
    }
}
