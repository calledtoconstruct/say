//! Local next-word model.
//!
//! Each word is a vector made from its role, its categories, and the letters
//! it speaks. Finished sentences add weight to a small directed graph stored
//! on this computer. Prediction adds the graph, the vectors, and the sentence
//! grammar. No account and no network.

use std::collections::{BTreeMap, BTreeSet};

use crate::board::{self, Word};

const BINS: usize = 24;
const WIDTH: usize = 7 + BINS;
/// A repeated line can raise a link, and then it stops so one phrase cannot
/// drown the links that shipped with the words.
const LEARNED_CAP: f64 = 8.0;

pub type Graph = BTreeMap<(String, String), f64>;

/// Record each step of a finished line. Returns true when the graph changed.
pub fn learn(graph: &mut Graph, ids: &[&str]) -> bool {
    let mut changed = false;
    for pair in ids.windows(2) {
        if board::word_by_id(pair[0]).is_none() || board::word_by_id(pair[1]).is_none() {
            continue;
        }
        let key = (pair[0].to_string(), pair[1].to_string());
        let weight = graph.entry(key).or_insert(0.0);
        let next = (*weight + 1.0).min(LEARNED_CAP);
        if (next - *weight).abs() > f64::EPSILON {
            *weight = next;
            changed = true;
        }
    }
    changed
}

/// Graph guesses, with a Llama list in front when that test has answered.
pub fn mix<'a>(
    ids: &[&str],
    limit: usize,
    hidden: &BTreeSet<String>,
    graph: &Graph,
    skill: u8,
    llama: Option<&[String]>,
) -> Vec<&'a Word> {
    let mut out = Vec::new();
    if let Some(ranked) = llama {
        for id in ranked {
            let Some(word) = board::word_by_id(id) else {
                continue;
            };
            if hidden.contains(word.id) || !board::in_skill(word, skill) {
                continue;
            }
            if out.iter().any(|have: &&Word| have.id == word.id) {
                continue;
            }
            out.push(word);
            if out.len() == limit {
                return out;
            }
        }
    }
    for word in predict(ids, limit + out.len(), hidden, graph, skill) {
        if out.iter().any(|have| have.id == word.id) {
            continue;
        }
        out.push(word);
        if out.len() == limit {
            break;
        }
    }
    out
}

pub fn predict<'a>(
    ids: &[&str],
    limit: usize,
    hidden: &BTreeSet<String>,
    graph: &Graph,
    skill: u8,
) -> Vec<&'a Word> {
    let last = ids.last().and_then(|id| board::word_by_id(id));
    let expect = board::expect_weights(last.map(|word| word.role));
    let topic = sentence_vector(ids);
    let used: BTreeSet<&str> = ids.iter().copied().collect();
    let mut scored: Vec<(&Word, f64)> = Vec::new();
    for word in board::words() {
        if hidden.contains(word.id) || !board::in_skill(word, skill) {
            continue;
        }
        if used.contains(word.id) && !board::allows_repeat(word) {
            continue;
        }
        let authored = last
            .and_then(|from| {
                from.next
                    .iter()
                    .find(|(id, _)| *id == word.id)
                    .map(|(_, weight)| *weight)
            })
            .unwrap_or(0.0);
        let learned = last
            .and_then(|from| graph.get(&(from.id.to_string(), word.id.to_string())))
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, LEARNED_CAP);
        let earlier_learned = if ids.len() >= 2 {
            graph
                .get(&(ids[ids.len() - 2].to_string(), word.id.to_string()))
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, LEARNED_CAP)
        } else {
            0.0
        };
        let similar = if ids.is_empty() {
            0.0
        } else {
            dot(&topic, &features(word))
        };
        let mut grammar = 0.0;
        for (index, weight) in expect.iter().enumerate() {
            grammar += weight * word.vec[index];
        }
        let score = authored * 2.0
            + learned
            + board::earlier_link(ids, word.id) * 0.8
            + earlier_learned * 0.35
            + board::topic_bonus(ids, word.id)
            + similar * 2.5
            + grammar
            + board::join_bonus(last, word)
            + board::tail_bonus(last, word, authored, learned)
            + if last.is_some() { 0.0 } else { word.starter };
        if score <= 0.0 {
            continue;
        }
        scored.push((word, score));
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.id.cmp(b.0.id)));
    scored.into_iter().take(limit).map(|(word, _)| word).collect()
}

fn features(word: &Word) -> [f64; WIDTH] {
    let mut vector = [0.0; WIDTH];
    vector[..7].copy_from_slice(&word.vec);
    let say = word.say.to_ascii_lowercase();
    add_text(&mut vector, &say);
    for cat in board::categories() {
        if cat.words.contains(&word.id) {
            add_bin(&mut vector, cat.id);
        }
    }
    normalize(&mut vector);
    vector
}

fn sentence_vector(ids: &[&str]) -> [f64; WIDTH] {
    let mut vector = [0.0; WIDTH];
    let last = ids.len().saturating_sub(1);
    for (index, id) in ids.iter().enumerate() {
        let Some(word) = board::word_by_id(id) else {
            continue;
        };
        let weight = if index == last { 2.0 } else { 1.0 };
        for (slot, value) in vector.iter_mut().zip(features(word)) {
            *slot += weight * value;
        }
    }
    normalize(&mut vector);
    vector
}

fn add_text(vector: &mut [f64; WIDTH], text: &str) {
    let bytes = text.as_bytes();
    if bytes.len() < 3 {
        add_bin(vector, text);
        return;
    }
    for start in 0..=bytes.len() - 3 {
        let gram = std::str::from_utf8(&bytes[start..start + 3]).unwrap_or("");
        add_bin(vector, gram);
    }
}

fn add_bin(vector: &mut [f64; WIDTH], text: &str) {
    vector[7 + (fnv(text) as usize % BINS)] += 1.0;
}

fn fnv(text: &str) -> u32 {
    let mut hash = 2_166_136_261u32;
    for byte in text.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn normalize(vector: &mut [f64]) {
    let scale = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
    if scale > 0.0 {
        for value in vector {
            *value /= scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn want_follows_i_and_a_learned_link_can_lead() {
        let hidden = BTreeSet::new();
        let empty = Graph::new();
        let after_i = predict(&["i"], 4, &hidden, &empty, 1);
        assert_eq!(after_i[0].id, "want", "{}", after_i[0].id);
        let mut graph = Graph::new();
        graph.insert(("want".to_string(), "outside".to_string()), 8.0);
        let after_want = predict(&["want"], 4, &hidden, &graph, 1);
        assert_eq!(after_want[0].id, "outside", "{}", after_want[0].id);
        let mut blocked = BTreeSet::new();
        blocked.insert("water".to_string());
        let foods = predict(&["want"], 8, &blocked, &empty, 1);
        assert!(foods.iter().all(|word| word.id != "water"));
    }

    #[test]
    fn learning_caps_a_repeated_pair() {
        let mut graph = Graph::new();
        let line = ["i", "want", "water"];
        for _ in 0..20 {
            learn(&mut graph, &line);
        }
        assert_eq!(graph.get(&("i".to_string(), "want".to_string())), Some(&8.0));
        assert_eq!(
            graph.get(&("want".to_string(), "water".to_string())),
            Some(&8.0)
        );
    }
}
