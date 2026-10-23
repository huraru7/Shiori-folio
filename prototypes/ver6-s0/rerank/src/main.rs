//! Ver6.0 段階0の試作1: リランカー(japanese-reranker-xsmall-v2)をcandleで動かし、
//! dump_pairs.pyが書き出したPython版の点数・トークンのID・時間と比べる。
//!
//! 実行: cargo run --release -- --model <モデルのフォルダ> [--data data/pairs.json] [--batch 1] [--workers 16] [--only timing]
//! モデルのフォルダには config.json・model.safetensors・tokenizer.json を置く。

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use candle_core::{DType, Device, IndexOp, Module, Tensor};
use candle_nn::{LayerNorm, Linear, VarBuilder, layer_norm_no_bias, linear, linear_no_bias};
use candle_transformers::models::modernbert::{Config, ModernBert};
use rayon::prelude::*;
use serde::Deserialize;
use tokenizers::Tokenizer;

/// config.jsonのうち、candleのConfigを組むのに要る分だけ読む。
/// candleのConfigをそのまま読むと、label2idの値(数値)が文字列を期待する型と合わず失敗するため。
#[derive(Deserialize)]
struct HfConfig {
    vocab_size: usize,
    hidden_size: usize,
    num_hidden_layers: usize,
    num_attention_heads: usize,
    intermediate_size: usize,
    max_position_embeddings: usize,
    norm_eps: f64,
    pad_token_id: u32,
    global_attn_every_n_layers: usize,
    global_rope_theta: f64,
    local_attention: usize,
    local_rope_theta: f64,
    classifier_pooling: String,
}

/// candleのModernBertForSequenceClassificationは最後にソフトマックスを掛け、出力が1つの
/// このモデルでは常に1.0になる。そこで本体だけcandleのものを使い、頭の部分は自前で組む
/// (Hugging FaceのModernBertPredictionHead+classifierと同じ: dense→GELU→LayerNorm→Linear)。
struct Reranker {
    model: ModernBert,
    head_dense: Linear,
    head_norm: LayerNorm,
    classifier: Linear,
    pad_id: u32,
    device: Device,
}

impl Reranker {
    fn load(dir: &PathBuf, device: &Device) -> Result<Self> {
        let hf: HfConfig = serde_json::from_str(&std::fs::read_to_string(dir.join("config.json"))?)?;
        if hf.classifier_pooling != "cls" {
            return Err(anyhow!("classifier_poolingがclsではない: {}", hf.classifier_pooling));
        }
        let config = Config {
            vocab_size: hf.vocab_size,
            hidden_size: hf.hidden_size,
            num_hidden_layers: hf.num_hidden_layers,
            num_attention_heads: hf.num_attention_heads,
            intermediate_size: hf.intermediate_size,
            max_position_embeddings: hf.max_position_embeddings,
            layer_norm_eps: hf.norm_eps,
            pad_token_id: hf.pad_token_id,
            global_attn_every_n_layers: hf.global_attn_every_n_layers,
            global_rope_theta: hf.global_rope_theta,
            local_attention: hf.local_attention,
            local_rope_theta: hf.local_rope_theta,
            classifier_config: None,
        };
        let weights = dir.join("model.safetensors");
        // SAFETY: 試作では、実行中にモデルのファイルが書き換えられない前提でmmapする
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights], DType::F32, device)? };
        let model = ModernBert::load(vb.clone(), &config)?;
        let h = config.hidden_size;
        let head_dense = linear_no_bias(h, h, vb.pp("head.dense"))?;
        let head_norm = layer_norm_no_bias(h, config.layer_norm_eps, vb.pp("head.norm"))?;
        let classifier = linear(h, 1, vb.pp("classifier"))?;
        Ok(Self { model, head_dense, head_norm, classifier, pad_id: config.pad_token_id, device: device.clone() })
    }

    /// トークンのIDの列をまとめて点数づけし、生の値(シグモイド前)を返す。
    fn score_batch(&self, batch: &[&Vec<u32>]) -> Result<Vec<f32>> {
        let max_len = batch.iter().map(|ids| ids.len()).max().unwrap_or(0);
        let mut ids = Vec::with_capacity(batch.len() * max_len);
        let mut mask = Vec::with_capacity(batch.len() * max_len);
        for row in batch {
            ids.extend(row.iter().copied());
            ids.extend(std::iter::repeat_n(self.pad_id, max_len - row.len()));
            mask.extend(std::iter::repeat_n(1u32, row.len()));
            mask.extend(std::iter::repeat_n(0u32, max_len - row.len()));
        }
        let ids = Tensor::from_vec(ids, (batch.len(), max_len), &self.device)?;
        let mask = Tensor::from_vec(mask, (batch.len(), max_len), &self.device)?;
        let hidden = self.model.forward(&ids, &mask)?;
        let cls = hidden.i((.., 0, ..))?.contiguous()?;
        let x = self.head_dense.forward(&cls)?.gelu_erf()?;
        let x = self.head_norm.forward(&x)?;
        let logits = self.classifier.forward(&x)?.squeeze(1)?;
        Ok(logits.to_vec1::<f32>()?)
    }

    /// 長さの近いもの同士でまとめ、詰め物を減らしてから点数づけする。返す順は入力と同じ。
    fn score_all(&self, encoded: &[Vec<u32>], batch_size: usize) -> Result<Vec<f32>> {
        let mut order: Vec<usize> = (0..encoded.len()).collect();
        order.sort_by_key(|&i| encoded[i].len());
        // candleのCPUの行列の掛け算は、この大きさでは並列にならない。まとまりごとに並列に計算する
        let scored: Vec<Vec<(usize, f32)>> = order
            .par_chunks(batch_size)
            .map(|chunk| {
                let rows: Vec<&Vec<u32>> = chunk.iter().map(|&i| &encoded[i]).collect();
                Ok(chunk.iter().copied().zip(self.score_batch(&rows)?).collect())
            })
            .collect::<Result<_>>()?;
        let mut out = vec![0f32; encoded.len()];
        for (i, s) in scored.into_iter().flatten() {
            out[i] = s;
        }
        Ok(out)
    }
}

#[derive(Deserialize)]
struct Pairs {
    queries: Vec<QueryCase>,
    timing: Vec<TimingCase>,
}

#[derive(Deserialize)]
struct QueryCase {
    id: String,
    kind: String,
    query: String,
    passages: Vec<String>,
    py_scores: Vec<f64>,
    py_raw: Vec<f64>,
    py_seconds: f64,
    token_ids: Vec<Vec<u32>>,
}

#[derive(Deserialize)]
struct TimingCase {
    id: String,
    query: String,
    passages: Vec<String>,
    py_scores: Vec<f64>,
    py_seconds: f64,
}

fn sigmoid(x: f32) -> f64 {
    1.0 / (1.0 + (-(x as f64)).exp())
}

/// 点数の高い順の並び(添字の列)
fn ranking(scores: &[f64]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    idx.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    idx
}

/// スピアマンの順位相関(同点は考えない。点数は連続値なので実質起きない)
fn spearman(a: &[f64], b: &[f64]) -> f64 {
    let rank_of = |s: &[f64]| {
        let mut r = vec![0f64; s.len()];
        for (pos, i) in ranking(s).into_iter().enumerate() {
            r[i] = pos as f64;
        }
        r
    };
    let (ra, rb) = (rank_of(a), rank_of(b));
    let n = a.len() as f64;
    let d2: f64 = ra.iter().zip(&rb).map(|(x, y)| (x - y).powi(2)).sum();
    1.0 - 6.0 * d2 / (n * (n * n - 1.0))
}

fn encode_pairs(tok: &Tokenizer, query: &str, passages: &[String]) -> Result<Vec<Vec<u32>>> {
    passages
        .iter()
        .map(|p| {
            let enc = tok.encode((query, p.as_str()), true).map_err(|e| anyhow!("トークン化に失敗: {e}"))?;
            Ok(enc.get_ids().to_vec())
        })
        .collect()
}

fn main() -> Result<()> {
    let mut args: HashMap<String, String> = HashMap::new();
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let v = it.next().with_context(|| format!("{k} の値が無い"))?;
        args.insert(k.trim_start_matches("--").to_string(), v);
    }
    let model_dir = PathBuf::from(args.get("model").context("--model が要る")?);
    let data_path = PathBuf::from(args.get("data").map(String::as_str).unwrap_or("data/pairs.json"));
    let batch_size: usize = args.get("batch").map(|s| s.parse()).transpose()?.unwrap_or(16);
    // --only timing で、速さの比較(候補200件)だけを行う
    let only_timing = args.get("only").map(String::as_str) == Some("timing");

    // --workers N: 候補のまとまりをN本のスレッドで並列に計算し、candleの中の行列の掛け算は
    // 1スレッドにする(両方が同じrayonのスレッドを取り合うと遅くなるため)。
    // candleは環境変数RAYON_NUM_THREADSで中のスレッド数を決めるので、何かを動かす前に設定する。
    let workers: Option<usize> = args.get("workers").map(|s| s.parse()).transpose()?;
    if workers.is_some() {
        // SAFETY: ほかのスレッドを作る前に、1回だけ設定する
        unsafe { std::env::set_var("RAYON_NUM_THREADS", "1") };
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers.unwrap_or(0))
        .build()?;
    pool.install(|| run(&model_dir, &data_path, batch_size, only_timing))
}

fn run(
    model_dir: &PathBuf,
    data_path: &PathBuf,
    batch_size: usize,
    only_timing: bool,
) -> Result<()> {
    let device = Device::Cpu;
    let t0 = Instant::now();
    let tok = Tokenizer::from_file(model_dir.join("tokenizer.json")).map_err(|e| anyhow!("{e}"))?;
    let reranker = Reranker::load(model_dir, &device)?;
    let load_secs = t0.elapsed().as_secs_f64();

    let pairs: Pairs = serde_json::from_str(&std::fs::read_to_string(data_path)?)?;

    // 初回の呼び出しの遅れを測定から外すため、1回だけ空回しする
    let warm = encode_pairs(&tok, "準備", &pairs.queries[0].passages[..4])?;
    reranker.score_all(&warm, batch_size)?;

    println!("モデルの読み込み: {load_secs:.2}秒 / まとめる数: {batch_size}");
    println!();
    println!("| 質問 | 種類 | 件数 | トークンID | 点数の差の最大 | 平均 | 生の値の差の最大 | 上位5件 | 順位相関 | Python秒 | candle秒 |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");

    let queries: &[QueryCase] = if only_timing { &[] } else { &pairs.queries };
    let mut all_max = 0f64;
    let mut all_sum = 0f64;
    let mut all_n = 0usize;
    let mut top5_ok = 0usize;
    let mut tok_ok = 0usize;
    let mut tok_n = 0usize;
    let (mut py_total, mut rs_total) = (0f64, 0f64);
    for q in queries {
        let encoded = encode_pairs(&tok, &q.query, &q.passages)?;
        for (mine, theirs) in encoded.iter().zip(&q.token_ids) {
            tok_n += 1;
            if mine == theirs {
                tok_ok += 1;
            }
        }
        let tok_same = encoded.iter().zip(&q.token_ids).all(|(a, b)| a == b);

        let t = Instant::now();
        let raw = reranker.score_all(&encoded, batch_size)?;
        let secs = t.elapsed().as_secs_f64();

        let scores: Vec<f64> = raw.iter().map(|&x| sigmoid(x)).collect();
        let diffs: Vec<f64> = scores.iter().zip(&q.py_scores).map(|(a, b)| (a - b).abs()).collect();
        let raw_max = raw.iter().zip(&q.py_raw).map(|(a, b)| (*a as f64 - b).abs()).fold(0f64, f64::max);
        let dmax = diffs.iter().copied().fold(0f64, f64::max);
        let dmean = diffs.iter().sum::<f64>() / diffs.len() as f64;
        let top5_same = ranking(&scores)[..5] == ranking(&q.py_scores)[..5];
        if top5_same {
            top5_ok += 1;
        }
        let rho = spearman(&scores, &q.py_scores);
        all_max = all_max.max(dmax);
        all_sum += diffs.iter().sum::<f64>();
        all_n += diffs.len();
        py_total += q.py_seconds;
        rs_total += secs;
        println!(
            "| {} | {} | {} | {} | {:.2e} | {:.2e} | {:.2e} | {} | {:.4} | {:.2} | {:.2} |",
            q.id,
            q.kind,
            q.passages.len(),
            if tok_same { "一致" } else { "不一致" },
            dmax,
            dmean,
            raw_max,
            if top5_same { "一致" } else { "不一致" },
            rho,
            q.py_seconds,
            secs
        );
    }
    println!();
    println!(
        "全体: トークンID一致 {tok_ok}/{tok_n}、点数の差の最大 {all_max:.2e}・平均 {:.2e}、上位5件の一致 {top5_ok}/{}、時間の合計 Python {py_total:.1}秒・candle {rs_total:.1}秒",
        all_sum / all_n as f64,
        queries.len()
    );

    println!();
    println!("速さ(本番の候補の上限と同じ件数)");
    println!();
    println!("| 質問 | 件数 | 点数の差の最大 | Python秒 | candle秒 |");
    println!("|---|---|---|---|---|");
    for t in &pairs.timing {
        let encoded = encode_pairs(&tok, &t.query, &t.passages)?;
        let start = Instant::now();
        let raw = reranker.score_all(&encoded, batch_size)?;
        let secs = start.elapsed().as_secs_f64();
        let dmax = raw.iter().zip(&t.py_scores).map(|(a, b)| (sigmoid(*a) - b).abs()).fold(0f64, f64::max);
        println!("| {} | {} | {:.2e} | {:.2} | {:.2} |", t.id, t.passages.len(), dmax, t.py_seconds, secs);
    }
    Ok(())
}
