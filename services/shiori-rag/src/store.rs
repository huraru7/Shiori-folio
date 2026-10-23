//! ベクトルの置き場(Python版のChromaDBのコレクションの代わり)。
//!
//! - 記事(メタデータの `source`)ごとに1ファイル。中身は、チャンクのID・本文・メタデータ(JSON)と
//!   ベクトル(f32の並び)。起動時に全部をメモリに読み込む。
//! - 書き換えは記事ごとに、一時ファイルに書いてから名前を付け替える(途中で止まっても壊れたファイルを残さない)。
//! - 検索は全件と比べる(厳密)。距離はChromaDBの既定と同じ「二乗ユークリッド距離」。
//! - SQLiteは使わない(Macのfskit経由のexFATで、SQLiteのロックが効かない事例があるため)。

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::filter::{Meta, Where, matches};

const MAGIC: &[u8; 6] = b"SRAG1\n";
const SHARD_EXT: &str = "bin";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    pub id: String,
    pub document: String,
    pub metadata: Meta,
    #[serde(skip)]
    pub embedding: Vec<f32>,
}

impl Record {
    pub fn source(&self) -> &str {
        self.metadata.get("source").and_then(Value::as_str).unwrap_or("")
    }
}

pub struct Collection {
    dir: PathBuf,
    /// source → その記事のチャンク(登録した順)
    by_source: BTreeMap<String, Vec<Record>>,
}

/// 読み込みの結果。壊れていたファイルの数(その記事は次の同期で作り直す)を返す。
pub struct LoadReport {
    pub broken_files: usize,
}

impl Collection {
    /// 置き場のフォルダから読み込む。フォルダが無ければ作る。
    pub fn open(dir: &Path) -> io::Result<(Self, LoadReport)> {
        fs::create_dir_all(dir.join("chunks"))?;
        let mut by_source = BTreeMap::new();
        let mut broken = 0;
        for entry in fs::read_dir(dir.join("chunks"))?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(SHARD_EXT) {
                continue;
            }
            match read_shard(&path) {
                Ok(records) if !records.is_empty() => {
                    let source = records[0].source().to_string();
                    by_source.insert(source, records);
                }
                Ok(_) => {}
                Err(e) => {
                    crate::log::warn(
                        crate::errors::RAG_003.code,
                        crate::errors::RAG_003.message,
                        &[("場所", &path.display()), ("理由", &e)],
                    );
                    let _ = fs::remove_file(&path);
                    broken += 1;
                }
            }
        }
        Ok((Self { dir: dir.to_path_buf(), by_source }, LoadReport { broken_files: broken }))
    }

    pub fn count(&self) -> usize {
        self.by_source.values().map(Vec::len).sum()
    }

    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.by_source.values().flatten()
    }

    pub fn sources(&self) -> impl Iterator<Item = &String> {
        self.by_source.keys()
    }

    pub fn has_source(&self, source: &str) -> bool {
        self.by_source.contains_key(source)
    }

    /// その記事のチャンクを全部消す。
    pub fn delete_source(&mut self, source: &str) -> io::Result<()> {
        if self.by_source.remove(source).is_some() {
            let path = self.shard_path(source);
            match fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    /// 記事のチャンクを、渡したものに丸ごと置き換える(Python版の「同じsourceを消してからupsert」と同じ)。
    pub fn replace_source(&mut self, source: &str, records: Vec<Record>) -> io::Result<()> {
        if records.is_empty() {
            return self.delete_source(source);
        }
        write_shard(&self.shard_path(source), &records)?;
        self.by_source.insert(source.to_string(), records);
        Ok(())
    }

    /// 1件をIDで追加・上書きする(メモの即時登録 `/add_document` 向け)。
    pub fn upsert(&mut self, record: Record) -> io::Result<()> {
        let source = record.source().to_string();
        let mut touched = vec![];
        for (s, recs) in self.by_source.iter_mut() {
            let before = recs.len();
            recs.retain(|r| r.id != record.id);
            if recs.len() != before && *s != source {
                touched.push(s.clone());
            }
        }
        self.by_source.entry(source.clone()).or_default().push(record);
        touched.push(source);
        for s in touched {
            let recs = self.by_source.get(&s).cloned().unwrap_or_default();
            if recs.is_empty() {
                self.by_source.remove(&s);
                let _ = fs::remove_file(self.shard_path(&s));
            } else {
                write_shard(&self.shard_path(&s), &recs)?;
            }
        }
        Ok(())
    }

    /// 全件と比べ、近い順に最大n件を返す(絞り込みに合うものだけ)。
    pub fn query(&self, embedding: &[f32], n: usize, filter: Option<&Where>) -> Vec<(&Record, f64)> {
        let mut scored: Vec<(&Record, f64)> = self
            .records()
            .filter(|r| matches(&r.metadata, filter))
            .map(|r| (r, squared_l2(embedding, &r.embedding)))
            .collect();
        scored.sort_by(|a, b| a.1.total_cmp(&b.1));
        scored.truncate(n);
        scored
    }

    fn shard_path(&self, source: &str) -> PathBuf {
        let hash = Sha256::digest(source.as_bytes());
        self.dir.join("chunks").join(format!("{}.{SHARD_EXT}", &hex::encode(hash)[..32]))
    }
}

pub fn squared_l2(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() {
        return f64::MAX;
    }
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>() as f64
}

fn write_shard(path: &Path, records: &[Record]) -> io::Result<()> {
    let json = serde_json::to_vec(records).map_err(io::Error::other)?;
    let dim = records.first().map_or(0, |r| r.embedding.len());
    if records.iter().any(|r| r.embedding.len() != dim) {
        return Err(io::Error::other("ベクトルの次元がそろっていない"));
    }
    let mut buf = Vec::with_capacity(MAGIC.len() + 8 + json.len() + records.len() * dim * 4);
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&(json.len() as u32).to_le_bytes());
    buf.extend_from_slice(&json);
    buf.extend_from_slice(&(dim as u32).to_le_bytes());
    for r in records {
        for x in &r.embedding {
            buf.extend_from_slice(&x.to_le_bytes());
        }
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&buf)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

fn read_shard(path: &Path) -> io::Result<Vec<Record>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.read_to_end(&mut bytes)?;
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "形が正しくない");
    if !bytes.starts_with(MAGIC) {
        return Err(bad());
    }
    let mut pos = MAGIC.len();
    let mut take = |n: usize| -> io::Result<&[u8]> {
        let s = bytes.get(pos..pos + n).ok_or_else(bad)?;
        pos += n;
        Ok(s)
    };
    let json_len = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
    let mut records: Vec<Record> = serde_json::from_slice(take(json_len)?).map_err(|_| bad())?;
    let dim = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
    for r in records.iter_mut() {
        r.embedding = take(dim * 4)?.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    }
    if pos != bytes.len() {
        return Err(bad());
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(id: &str, source: &str, v: [f32; 2]) -> Record {
        Record {
            id: id.into(),
            document: format!("本文{id}"),
            metadata: json!({"source": source, "status": "new"}).as_object().unwrap().clone(),
            embedding: v.to_vec(),
        }
    }

    #[test]
    fn 書いて読み直す() {
        let dir = tempfile::tempdir().unwrap();
        let (mut c, _) = Collection::open(dir.path()).unwrap();
        c.replace_source("a.md", vec![rec("a-0", "a.md", [0.0, 0.0]), rec("a-1", "a.md", [1.0, 0.0])]).unwrap();
        c.replace_source("b.md", vec![rec("b-0", "b.md", [0.0, 3.0])]).unwrap();
        c.upsert(rec("memo-1", "memo.md", [0.0, 1.0])).unwrap();
        let (c2, report) = Collection::open(dir.path()).unwrap();
        assert_eq!(report.broken_files, 0);
        assert_eq!(c2.count(), 4);
        let hits = c2.query(&[0.0, 0.9], 2, None);
        assert_eq!(hits[0].0.id, "memo-1");
        assert!((hits[0].1 - 0.01).abs() < 1e-6);
        assert_eq!(hits[1].0.id, "a-0");
        let only_b = c2.query(&[0.0, 0.0], 10, Some(&Where::eq("source", "b.md")));
        assert_eq!(only_b.len(), 1);
    }

    #[test]
    fn 消すと壊れたファイル() {
        let dir = tempfile::tempdir().unwrap();
        let (mut c, _) = Collection::open(dir.path()).unwrap();
        c.replace_source("a.md", vec![rec("a-0", "a.md", [0.0, 0.0])]).unwrap();
        c.replace_source("b.md", vec![rec("b-0", "b.md", [0.0, 0.0])]).unwrap();
        c.delete_source("a.md").unwrap();
        // b.mdのファイルを壊す
        let shard = c.shard_path("b.md");
        fs::write(&shard, b"SRAG1\nxx").unwrap();
        let (c2, report) = Collection::open(dir.path()).unwrap();
        assert_eq!(report.broken_files, 1);
        assert_eq!(c2.count(), 0);
        assert!(!shard.exists());
    }

    #[test]
    fn 同じidの上書き() {
        let dir = tempfile::tempdir().unwrap();
        let (mut c, _) = Collection::open(dir.path()).unwrap();
        c.upsert(rec("m", "x.md", [0.0, 0.0])).unwrap();
        c.upsert(rec("m", "y.md", [1.0, 0.0])).unwrap();
        assert_eq!(c.count(), 1);
        assert!(!c.has_source("x.md"));
        let (c2, _) = Collection::open(dir.path()).unwrap();
        assert_eq!(c2.count(), 1);
    }
}
