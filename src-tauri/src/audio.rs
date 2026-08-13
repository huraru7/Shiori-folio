//! マイク録音(cpal)とWAVエンコード(hound)。
//!
//! cpal::StreamはWindows(WASAPI)ではSendを実装しないため、Tauriのstateに
//! 直接持たせることができない。そのため録音の開始〜停止までを専用スレッドに
//! 閉じ込め、コマンド用チャンネル(mpsc)経由でやり取りする。ホットキー経由・
//! VoiceBarのマイクボタン経由のどちらから呼ばれても、この実装を共有する。

use std::io::Cursor;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

enum AudioCommand {
    Start(Sender<Result<(), String>>),
    Stop(Sender<Result<Vec<u8>, String>>),
}

pub struct RecordingState {
    cmd_tx: Sender<AudioCommand>,
    recording: Arc<Mutex<bool>>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingState {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<AudioCommand>();
        let recording = Arc::new(Mutex::new(false));
        let recording_for_thread = recording.clone();

        std::thread::spawn(move || {
            // (Stream, サンプルバッファ, サンプルレート, チャンネル数)
            let mut current: Option<(cpal::Stream, Arc<Mutex<Vec<f32>>>, u32, u16)> = None;

            for cmd in cmd_rx {
                match cmd {
                    AudioCommand::Start(reply) => {
                        let result = build_and_play_stream();
                        match result {
                            Ok(session) => {
                                current = Some(session);
                                if let Ok(mut r) = recording_for_thread.lock() {
                                    *r = true;
                                }
                                let _ = reply.send(Ok(()));
                            }
                            Err(e) => {
                                let _ = reply.send(Err(e));
                            }
                        }
                    }
                    AudioCommand::Stop(reply) => {
                        let result = (|| -> Result<Vec<u8>, String> {
                            let (stream, buffer, sample_rate, channels) =
                                current.take().ok_or_else(|| "録音していません".to_string())?;
                            let _ = stream.pause();
                            drop(stream);
                            let samples = buffer.lock().map_err(|e| e.to_string())?.clone();
                            let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
                            eprintln!(
                                "録音診断: サンプル数={} サンプルレート={}Hz チャンネル数={} 音量ピーク={:.4}",
                                samples.len(),
                                sample_rate,
                                channels,
                                peak
                            );
                            encode_wav(&samples, sample_rate, channels)
                        })();
                        if let Ok(mut r) = recording_for_thread.lock() {
                            *r = false;
                        }
                        let _ = reply.send(result);
                    }
                }
            }
        });

        Self { cmd_tx, recording }
    }

    pub fn is_recording(&self) -> bool {
        self.recording.lock().map(|r| *r).unwrap_or(false)
    }
}

fn build_and_play_stream() -> Result<(cpal::Stream, Arc<Mutex<Vec<f32>>>, u32, u16), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "入力デバイス(マイク)が見つかりません".to_string())?;
    eprintln!(
        "録音デバイス: {}",
        device.name().unwrap_or_else(|_| "(名前取得不可)".to_string())
    );
    let config = device
        .default_input_config()
        .map_err(|e| format!("マイク設定の取得に失敗: {e}"))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let buffer_for_stream = buffer.clone();
    let err_fn = |err| eprintln!("録音ストリームエラー: {err}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if let Ok(mut buf) = buffer_for_stream.lock() {
                    buf.extend_from_slice(data);
                }
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                if let Ok(mut buf) = buffer_for_stream.lock() {
                    buf.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                }
            },
            err_fn,
            None,
        ),
        other => return Err(format!("未対応のサンプル形式です: {other:?}")),
    }
    .map_err(|e| format!("録音ストリームの構築に失敗: {e}"))?;

    stream.play().map_err(|e| format!("録音開始に失敗: {e}"))?;
    Ok((stream, buffer, sample_rate, channels))
}

fn encode_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| format!("WAV書き出しの初期化に失敗: {e}"))?;
        for &sample in samples {
            let clamped = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(clamped)
                .map_err(|e| format!("WAV書き込みに失敗: {e}"))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("WAV確定に失敗: {e}"))?;
    }

    Ok(cursor.into_inner())
}

pub fn start(state: &RecordingState) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    state
        .cmd_tx
        .send(AudioCommand::Start(tx))
        .map_err(|e| format!("録音スレッドへの送信に失敗: {e}"))?;
    rx.recv().map_err(|e| format!("録音スレッドからの応答待ちに失敗: {e}"))?
}

pub fn stop_and_encode_wav(state: &RecordingState) -> Result<Vec<u8>, String> {
    let (tx, rx) = mpsc::channel();
    state
        .cmd_tx
        .send(AudioCommand::Stop(tx))
        .map_err(|e| format!("録音スレッドへの送信に失敗: {e}"))?;
    rx.recv().map_err(|e| format!("録音スレッドからの応答待ちに失敗: {e}"))?
}
