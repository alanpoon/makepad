// SentencePiece tokenizer for sherpa-onnx streaming Zipformer CTC.
// tokens.txt format: "<token> <id>" per line.
// CTC: blank=0, sos/eos=1, unk=2, byte fallbacks <0xNN>=3..258, text tokens=259+

pub struct ZipformerTokenizer {
    id_to_token: Vec<Option<String>>,
}

impl ZipformerTokenizer {
    pub fn load(tokens_path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(tokens_path)
            .map_err(|e| format!("failed to read tokens.txt: {}", e))?;

        let mut max_id = 0usize;
        let mut pairs: Vec<(usize, String)> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            if let Some(last_space) = line.rfind(' ') {
                let token = &line[..last_space];
                let id_str = &line[last_space + 1..];
                if let Ok(id) = id_str.parse::<usize>() {
                    max_id = max_id.max(id);
                    pairs.push((id, token.to_string()));
                }
            }
        }

        let mut id_to_token = vec![None; max_id + 1];
        for (id, token) in pairs {
            id_to_token[id] = Some(token);
        }

        Ok(Self { id_to_token })
    }

    /// CTC decode: remove consecutive duplicates, skip blank (0), sos/eos (1), unk (2).
    /// Byte fallback tokens (<0xNN>) are accumulated and decoded as UTF-8.
    /// ▁ prefix is replaced with a space character.
    pub fn decode(&self, ids: &[i32]) -> String {
        let mut result = String::new();
        let mut byte_buf: Vec<u8> = Vec::new();
        let mut prev_id = -1i32;

        for &id in ids {
            if id == prev_id { continue; }
            prev_id = id;
            if id == 0 || id == 1 || id == 2 { continue; }

            if let Some(Some(token)) = self.id_to_token.get(id as usize) {
                if token.starts_with("<0x") && token.ends_with('>') {
                    if let Ok(byte_val) = u8::from_str_radix(&token[3..token.len() - 1], 16) {
                        byte_buf.push(byte_val);
                        continue;
                    }
                }
                if !byte_buf.is_empty() {
                    result.push_str(&String::from_utf8_lossy(&byte_buf));
                    byte_buf.clear();
                }
                result.push_str(&token.replace('▁', " "));
            }
        }

        if !byte_buf.is_empty() {
            result.push_str(&String::from_utf8_lossy(&byte_buf));
        }

        result.trim().to_string()
    }
}
