//! リランカー(hotchpotch/japanese-reranker-xsmall-v2)をcandleで動かす。
//!
//! 段階0の試作(`prototypes/ver6-s0/rerank/`)で、Python版(sentence-transformersの `CrossEncoder`)と
//! トークンのID・点数・順位が一致することを確かめた作りをそのまま使う。
//! - candleの `ModernBertForSequenceClassification` は最後にソフトマックスを掛け、出力が1つのこの
//!   モデルでは常に1.0になる。本体(`ModernBert`)だけcandleのものを使い、頭の部分は自分で組む
//! - config.jsonは自前の型で読む(candleの `Config` は `label2id` の型が合わず読めない)
//! - 候補を1件ずつ並列に計算し、candleの中の行列の掛け算は1スレッドにする(取り合うと遅い)
//! - 点数はPython版と同じく、生の値にシグモイドを掛けた0〜1の値

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use candle_core::{DType, Device, IndexOp, Module, Tensor};
use candle_nn::{LayerNorm, Linear, VarBuilder, layer_norm_no_bias, linear, linear_no_bias};
use candle_transformers::models::modernbert::{Config, ModernBert};
use rayon::prelude::*;
use serde::Deserialize;
use tokenizers::Tokenizer;

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

pub struct Reranker {
    model: ModernBert,
    head_dense: Linear,
    head_norm: LayerNorm,
    classifier: Linear,
    tokenizer: Tokenizer,
    device: Device,
    pool: rayon::ThreadPool,
}

impl Reranker {
    /// モデルのフォルダ(`config.json`・`model.safetensors`・`tokenizer.json`)から読み込む。
    /// `workers` は候補を並列に計算するスレッドの数。
    pub fn load(dir: &Path, workers: usize) -> Result<Self> {
        let device = Device::Cpu;
        let hf: HfConfig = serde_json::from_str(
            &std::fs::read_to_string(dir.join("config.json")).with_context(|| format!("{}", dir.join("config.json").display()))?,
        )?;
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
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| anyhow!("tokenizer.json: {e}"))?;
        // モデルのファイルは小さい(147MB)ので、mmapせずに読み込む(SSDを抜くときに掴んだままにしない)
        let bytes = std::fs::read(dir.join("model.safetensors")).context("model.safetensors")?;
        let vb = VarBuilder::from_buffered_safetensors(bytes, DType::F32, &device)?;
        let model = ModernBert::load(vb.clone(), &config)?;
        let h = config.hidden_size;
        let head_dense = linear_no_bias(h, h, vb.pp("head.dense"))?;
        let head_norm = layer_norm_no_bias(h, config.layer_norm_eps, vb.pp("head.norm"))?;
        let classifier = linear(h, 1, vb.pp("classifier"))?;
        let pool = rayon::ThreadPoolBuilder::new().num_threads(workers.max(1)).build()?;
        let r = Self { model, head_dense, head_norm, classifier, tokenizer, device, pool };
        // 初回の計算の遅れを、最初の検索ではなく起動のときに済ませる
        r.rerank("準備", &["準備のための文".to_string()])?;
        Ok(r)
    }

    fn score_one(&self, ids: &[u32]) -> Result<f32> {
        let n = ids.len();
        let ids = Tensor::from_vec(ids.to_vec(), (1, n), &self.device)?;
        let mask = Tensor::from_vec(vec![1u32; n], (1, n), &self.device)?;
        let hidden = self.model.forward(&ids, &mask)?;
        let cls = hidden.i((.., 0, ..))?.contiguous()?;
        let x = self.head_dense.forward(&cls)?.gelu_erf()?;
        let x = self.head_norm.forward(&x)?;
        let logits = self.classifier.forward(&x)?.squeeze(1)?;
        Ok(logits.to_vec1::<f32>()?[0])
    }

    /// queryと各passageの関連度(0〜1)を、passagesと同じ順で返す。
    pub fn rerank(&self, query: &str, passages: &[String]) -> Result<Vec<f64>> {
        if passages.is_empty() {
            return Ok(Vec::new());
        }
        let encoded: Vec<Vec<u32>> = passages
            .iter()
            .map(|p| {
                self.tokenizer
                    .encode((query, p.as_str()), true)
                    .map(|e| e.get_ids().to_vec())
                    .map_err(|e| anyhow!("トークン化に失敗: {e}"))
            })
            .collect::<Result<_>>()?;
        // 長いものから先に配ると、最後に長い1件だけが残って待つことが減る
        let mut order: Vec<usize> = (0..encoded.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(encoded[i].len()));
        let scored: Vec<(usize, f32)> = self.pool.install(|| {
            order
                .par_iter()
                .with_max_len(1)
                .map(|&i| Ok((i, self.score_one(&encoded[i])?)))
                .collect::<Result<_>>()
        })?;
        let mut out = vec![0f64; encoded.len()];
        for (i, raw) in scored {
            out[i] = 1.0 / (1.0 + (-(raw as f64)).exp());
        }
        Ok(out)
    }
}
