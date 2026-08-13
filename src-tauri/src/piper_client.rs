//! piper-plus-cli(TTS)のサブプロセス呼び出しと再生(rodio)。
//!
//! つくよみちゃんの最新配布モデル(MB-iSTFT版, tsukuyomi-chan-6lang-fp16.onnx)は
//! speaker_embeddingの次元宣言(192)と実際にグラフが要求する次元(256)が食い違う
//! アーティファクト側の不具合があり、音声合成そのものが失敗する。piper-plus本体を
//! 最新コミットまで更新し、CUDA向けcuda featureを有効にしても解消しなかったため、
//! 不具合発生前のWavLM版モデル(tsukuyomi-wavlm-300epoch.onnx、HF commit
//! 778b68b00722cd0239db5d79154dc6dcaa4a9b73)にピン留めしている
//! (models/tts/tsukuyomi-chan.onnx / tsukuyomi-chan.json として配置)。
//! このモデルはspeaker_embedding入力自体を持たないため、この不具合の影響を受けない。
//!
//! 万一また別の不具合で合成が失敗しても、会話フロー自体は継続させたいため、
//! 呼び出し元(synthesize_speechコマンド)でエラーを握って警告ログのみ出す想定。

use std::io::BufReader;
use std::path::Path;
use std::process::Command;

// 発話パラメータ(length_scale=速度、noise_scale/noise_w=声の揺らぎ)。
// piper-plus-cliは1回ごとの呼び出しでこれらを上書きできるため、モデルや
// サーバーの再起動なしに「即時反映」できる(コントロールパネルの音声設定より)。
pub struct VoiceParams {
    pub length_scale: f64,
    pub noise_scale: f64,
    pub noise_w: f64,
}

pub fn synthesize_and_play(
    root: &Path,
    text: &str,
    model_path: &Path,
    config_path: &Path,
    voice: &VoiceParams,
) -> Result<(), String> {
    let exe = root
        .join("third_party/piper-plus/src/rust/target/release")
        .join(crate::exe_name("piper-plus-cli"));
    let tmp_dir = std::env::temp_dir();
    let out_filename = format!("shiori-tts-{}.wav", std::process::id());
    let out_path = tmp_dir.join(&out_filename);

    let status = Command::new(&exe)
        .current_dir(&tmp_dir)
        .arg("--model")
        .arg(model_path)
        .arg("--config")
        .arg(config_path)
        .arg("--text")
        .arg(text)
        .arg("--output-file")
        .arg(&out_filename)
        .arg("--device")
        .arg(if cfg!(target_os = "macos") { "coreml" } else { "cuda" })
        .arg("--language")
        .arg("ja")
        .arg("--length-scale")
        .arg(voice.length_scale.to_string())
        .arg("--noise-scale")
        .arg(voice.noise_scale.to_string())
        .arg("--noise-w")
        .arg(voice.noise_w.to_string())
        .status()
        .map_err(|e| format!("piper-plus-cliの起動に失敗: {e}"))?;

    if !status.success() {
        return Err(format!(
            "piper-plus-cliが失敗しました(exit code={:?})",
            status.code()
        ));
    }

    let result = play_wav_file(&out_path);
    let _ = std::fs::remove_file(&out_path);
    result
}

fn play_wav_file(path: &Path) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("WAVファイルを開けません: {e}"))?;
    let (_stream, stream_handle) =
        rodio::OutputStream::try_default().map_err(|e| format!("出力デバイスの取得に失敗: {e}"))?;
    let sink = rodio::Sink::try_new(&stream_handle).map_err(|e| format!("再生シンクの作成に失敗: {e}"))?;
    let source =
        rodio::Decoder::new(BufReader::new(file)).map_err(|e| format!("WAVの読み込みに失敗: {e}"))?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
