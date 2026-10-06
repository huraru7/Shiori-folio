//! whisper-server(whisper.cpp、POST /inference、multipart)への問い合わせ。

use std::time::Duration;

use serde::Deserialize;

#[derive(Deserialize)]
struct InferenceResponse {
    text: String,
}

pub fn transcribe(port: u16, wav_bytes: &[u8]) -> Result<String, String> {
    const BOUNDARY: &str = "----shiorifolioboundary";

    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
    body.extend_from_slice(wav_bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

    let url = format!("http://127.0.0.1:{port}/inference");
    let response: InferenceResponse = ureq::post(&url)
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .timeout(Duration::from_secs(60))
        .send_bytes(&body)
        .map_err(|e| format!("whisper-serverへの送信に失敗: {e}"))?
        .into_json()
        .map_err(|e| format!("whisper-server応答の解析に失敗: {e}"))?;

    let text = response.text.trim().to_string();
    if is_non_speech(&text) {
        return Err(format!("発話を認識できませんでした(whisperの出力: {text})"));
    }
    Ok(text)
}

// whisperは発話が無い・聞き取れない録音に対して、「[音声なし]」「(音楽)」
// 「[BLANK_AUDIO]」のような括弧書きの注記だけを返すことがある。これを発言として
// 送ると、詩織が意味のない応答を返してしまうため弾く(2026-10-06、Ver3.6の再測定で発見)。
fn is_non_speech(text: &str) -> bool {
    const BRACKETS: [(char, char); 4] = [('[', ']'), ('(', ')'), ('（', '）'), ('【', '】')];
    text.is_empty()
        || BRACKETS
            .iter()
            .any(|(open, close)| text.starts_with(*open) && text.ends_with(*close))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_speech_annotations_are_detected() {
        assert!(is_non_speech("[音声なし]"));
        assert!(is_non_speech("(音楽)"));
        assert!(is_non_speech("[BLANK_AUDIO]"));
        assert!(is_non_speech(""));
        assert!(!is_non_speech("この機能実装するか迷ってて"));
    }
}
