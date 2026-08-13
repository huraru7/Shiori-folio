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

    Ok(response.text.trim().to_string())
}
