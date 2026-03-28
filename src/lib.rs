use wasm_bindgen::prelude::*;
use std::collections::HashMap;

// Static word storage loaded via load_words
static mut WORDS: Vec<[u8; 5]> = Vec::new();

/// Load words into WASM memory. Words packed as flat bytes (count * 5 bytes).
#[wasm_bindgen]
pub fn load_words(words_flat: &[u8]) {
    let count = words_flat.len() / 5;
    unsafe {
        WORDS.clear();
        WORDS.reserve(count);
        for i in 0..count {
            let mut w = [0u8; 5];
            w.copy_from_slice(&words_flat[i * 5..(i + 1) * 5]);
            WORDS.push(w);
        }
    }
}

/// Compute Wordle pattern for guess vs answer.
/// Returns base-3 encoded pattern: pattern[i] * 3^i
/// 0=gray, 1=yellow, 2=green
#[wasm_bindgen]
pub fn compute_pattern(guess: &[u8], answer: &[u8]) -> u32 {
    let mut result = [0u8; 5];
    let mut answer_remaining = [0u8; 26]; // letter counts

    // Pass 1: exact matches (green)
    for i in 0..5 {
        if guess[i] == answer[i] {
            result[i] = 2;
        } else {
            answer_remaining[(answer[i] - b'A') as usize] += 1;
        }
    }

    // Pass 2: wrong position (yellow) or absent (gray)
    for i in 0..5 {
        if result[i] == 2 {
            continue;
        }
        let idx = (guess[i] - b'A') as usize;
        if answer_remaining[idx] > 0 {
            result[i] = 1;
            answer_remaining[idx] -= 1;
        }
    }

    // Encode as base-3
    let mut pattern: u32 = 0;
    let mut power: u32 = 1;
    for i in 0..5 {
        pattern += (result[i] as u32) * power;
        power *= 3;
    }
    pattern
}

/// Check if a candidate word is consistent with a guess+pattern.
fn matches_pattern(guess: &[u8; 5], pattern: u32, candidate: &[u8; 5]) -> bool {
    let computed = compute_pattern(guess, candidate);
    computed == pattern
}

/// Filter candidate indices that match the given pattern for the guess.
/// guess: 5 bytes, pattern: base-3 encoded, candidates: array of indices into WORDS
/// Returns indices of matching candidates.
#[wasm_bindgen]
pub fn filter_candidates(guess: &[u8], pattern: u32, candidates: &[u32]) -> Vec<u32> {
    let mut guess_arr = [0u8; 5];
    guess_arr.copy_from_slice(guess);

    let mut result = Vec::new();
    unsafe {
        for &cand_idx in candidates {
            let cand = &WORDS[cand_idx as usize];
            if matches_pattern(&guess_arr, pattern, cand) {
                result.push(cand_idx);
            }
        }
    }
    result
}

/// Compute entropy for a guess against a set of answer candidates.
fn entropy_single(guess: &[u8; 5], answers: &[u32]) -> f64 {
    if answers.is_empty() {
        return 0.0;
    }

    let mut buckets: HashMap<u32, u32> = HashMap::new();
    unsafe {
        for &ans_idx in answers {
            let answer = &WORDS[ans_idx as usize];
            let pattern = compute_pattern(guess, answer);
            *buckets.entry(pattern).or_insert(0) += 1;
        }
    }

    let total = answers.len() as f64;
    let mut entropy = 0.0;
    for &count in buckets.values() {
        let p = count as f64 / total;
        entropy -= p * p.log2();
    }
    entropy
}

/// For each guess, compute combined entropy across both boards.
/// Returns top_k results as [index, entropy_scaled, index, entropy_scaled, ...].
#[wasm_bindgen]
pub fn compute_entropy_top(
    guesses_flat: &[u8],    // n_guesses * 5 bytes
    n_guesses: usize,
    answers1: &[u32],
    answers2: &[u32],
    top_k: usize,
) -> Vec<u32> {
    let mut scores: Vec<(usize, u32)> = Vec::new();

    for i in 0..n_guesses {
        let mut guess = [0u8; 5];
        guess.copy_from_slice(&guesses_flat[i * 5..(i + 1) * 5]);

        let h1 = entropy_single(&guess, answers1);
        let h2 = entropy_single(&guess, answers2);
        let combined = h1 + h2;

        // Scale entropy by 1000 to store as u32
        let scaled = (combined * 1000.0) as u32;
        scores.push((i, scaled));
    }

    // Sort descending by entropy
    scores.sort_by(|a, b| b.1.cmp(&a.1));

    // Take top_k
    let k = top_k.min(scores.len());
    let mut result = Vec::with_capacity(k * 2);
    for i in 0..k {
        result.push(scores[i].0 as u32);
        result.push(scores[i].1);
    }
    result
}
