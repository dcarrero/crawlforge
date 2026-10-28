//! b-bit MinHash signature of a page's visible text, for `DUP-CONTENT-NEAR`.
//!
//! MinHash (Broder, 1997) estimates the Jaccard similarity of two sets: hash every element with
//! one function, keep the smallest value, and the chance that two sets keep the same one is
//! exactly their Jaccard similarity. With [`PERMUTATIONS`] functions the share of positions where
//! two signatures agree estimates it. b-bit MinHash (Li and König, *b-Bit Minwise Hashing*,
//! WWW 2010) keeps only the lowest byte of each minimum; two different minima then agree by chance
//! one time in 256, which the rule corrects for.
//!
//! **Why not SimHash**, which fits in one `INTEGER` and was tried first: a single 64-bit
//! fingerprint is too noisy an estimate to set the threshold where it is useful. Two pages that
//! share 90 % of their text land anywhere between 6 and 12 bits apart, and at 9 bits a crawl of
//! 200,000 pages already has some 35 pairs of unrelated pages that fall under it by chance. With
//! 128 positions the estimate's standard deviation at 0.9 is about 0.03, and unrelated pages agree
//! on 1 position in 256, so they never get near the threshold.
//!
//! The signature is 128 bytes per page — some 27 MB in a 216,000-page crawl whose file is 5 GB.
//!
//! The elements are **word 3-shingles**: three consecutive words, lowercased, split on anything
//! that is not alphanumeric. Single words would make two pages on the same topic look alike;
//! shingles also capture the order.
//!
//! The hasher runs in constant memory — the minima, the last two word hashes and the word being
//! read — so it is fed while `lol_html` streams the document and never sees the text whole.

/// Number of hash functions, and of bytes in the signature.
pub const PERMUTATIONS: usize = 128;

/// Below this many shingles no signature is stored.
///
/// With a handful of shingles a single changed word moves a large share of them, so short texts
/// give signatures that say little; and short pages already have their own rule,
/// `CONTENT-THIN`. Fifty shingles is a fifty-two word text.
pub const MIN_SHINGLES: u32 = 50;

/// SplitMix64, to derive the hash functions' parameters at compile time.
const fn splitmix(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `(a, b)` of each function `h(x) = a·x + b mod 2³²`, `a` odd.
///
/// With `a` odd each one is a bijection of the 32-bit space, a true permutation, and the shingles
/// it is applied to are already uniform hashes, so the minimum of each lands on a uniformly chosen
/// shingle. Thirty-two bits rather than sixty-four because that is what vectorises: a 64-bit
/// multiply-shift version cost twice as much per page measured on an Apple M-series CPU
/// (about 0.20 ms against 0.04 ms on a 2,700-word article) because NEON has no 64-bit vector
/// multiply.
/// The estimator's accuracy is checked by `the_estimator_is_unbiased_across_many_pairs`.
///
/// **Fixed forever.** Changing a constant here changes every signature, and two crawls from
/// different binaries would stop being comparable. If it ever has to change, it is a new column.
const MUL: [u32; PERMUTATIONS] = {
    let mut out = [0u32; PERMUTATIONS];
    let mut i = 0;
    while i < PERMUTATIONS {
        out[i] = splitmix(2 * i as u64) as u32 | 1;
        i += 1;
    }
    out
};
const ADD: [u32; PERMUTATIONS] = {
    let mut out = [0u32; PERMUTATIONS];
    let mut i = 0;
    while i < PERMUTATIONS {
        out[i] = splitmix(2 * i as u64 + 1) as u32;
        i += 1;
    }
    out
};

/// Streaming signature builder. Feed it text with [`MinHasher::push`] and close it with
/// [`MinHasher::finish`].
#[derive(Debug)]
pub struct MinHasher {
    mins: [u32; PERMUTATIONS],
    /// Hashes of the two previous words, oldest first. `filled` says how many are valid.
    window: [u64; 2],
    filled: u8,
    shingles: u32,
    /// The word being read. Kept across chunks because `lol_html` may cut a text node anywhere,
    /// even inside a word, and a word split in two would change the signature depending on
    /// where the parser's buffers happened to end.
    word: String,
}

// By hand because `Default` is only derived for arrays of up to 32 elements.
impl Default for MinHasher {
    fn default() -> Self {
        Self {
            mins: [u32::MAX; PERMUTATIONS],
            window: [0; 2],
            filled: 0,
            shingles: 0,
            word: String::new(),
        }
    }
}

impl MinHasher {
    /// Feeds a chunk of text. `end_of_node` closes the word in progress: two text nodes are two
    /// words even if no space separates them (`<b>uno</b>dos`), whatever chunks they arrive in.
    pub fn push(&mut self, chunk: &str, end_of_node: bool) {
        for c in chunk.chars() {
            // ASCII first: it is most of the text even in Spanish, and `to_lowercase` builds an
            // iterator per character.
            if c.is_ascii_alphanumeric() {
                self.word.push(c.to_ascii_lowercase());
            } else if !c.is_ascii() && c.is_alphanumeric() {
                self.word.extend(c.to_lowercase());
            } else {
                self.end_word();
            }
        }
        if end_of_node {
            self.end_word();
        }
    }

    fn end_word(&mut self) {
        if self.word.is_empty() {
            return;
        }
        let h = xxhash_rust::xxh3::xxh3_64(self.word.as_bytes());
        self.word.clear();
        if self.filled < 2 {
            self.window[self.filled as usize] = h;
            self.filled += 1;
            return;
        }
        let mut bytes = [0u8; 24];
        bytes[..8].copy_from_slice(&self.window[0].to_le_bytes());
        bytes[8..16].copy_from_slice(&self.window[1].to_le_bytes());
        bytes[16..].copy_from_slice(&h.to_le_bytes());
        let shingle = xxhash_rust::xxh3::xxh3_64(&bytes);
        let x = (shingle ^ shingle >> 32) as u32;
        // Written as three plain arrays so that the compiler turns it into vector code.
        for ((min, a), b) in self.mins.iter_mut().zip(MUL.iter()).zip(ADD.iter()) {
            *min = (*min).min(a.wrapping_mul(x).wrapping_add(*b));
        }
        self.window = [self.window[1], h];
        self.shingles += 1;
    }

    /// The signature —the lowest byte of each minimum— or `None` if the text had fewer than
    /// [`MIN_SHINGLES`] shingles.
    pub fn finish(mut self) -> Option<Vec<u8>> {
        self.end_word();
        if self.shingles < MIN_SHINGLES {
            return None;
        }
        Some(self.mins.iter().map(|m| *m as u8).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(text: &str) -> Option<Vec<u8>> {
        let mut h = MinHasher::default();
        h.push(text, true);
        h.finish()
    }

    /// Share of positions where two signatures agree. The rule corrects it for chance; the raw
    /// share is enough to test the hasher.
    fn agreement(a: &[u8], b: &[u8]) -> f64 {
        a.iter().zip(b).filter(|(x, y)| x == y).count() as f64 / PERMUTATIONS as f64
    }

    /// A deterministic text of `n` distinct words, so that no two shingles repeat.
    fn text(n: usize, seed: &str) -> String {
        (0..n).map(|i| format!("{seed}{i}")).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn a_short_text_has_no_signature() {
        // 51 words are 49 shingles, one short of the minimum; 52 make 50.
        assert_eq!(signature(&text(51, "w")), None);
        assert_eq!(signature(&text(52, "w")).map(|s| s.len()), Some(PERMUTATIONS));
    }

    #[test]
    fn case_and_punctuation_do_not_change_the_signature() {
        let base = text(200, "palabra");
        let noisy = base.replace(' ', ", ").to_uppercase();
        assert_eq!(signature(&base), signature(&noisy));
    }

    #[test]
    fn a_word_cut_across_chunks_counts_once() {
        let base = text(200, "w");
        let mut h = MinHasher::default();
        // Cut in the middle of a word, inside the same text node.
        let (a, b) = base.split_at(base.len() / 2 + 1);
        h.push(a, false);
        h.push(b, true);
        assert_eq!(h.finish(), signature(&base));
    }

    #[test]
    fn the_end_of_a_text_node_ends_the_word() {
        let mut joined = MinHasher::default();
        joined.push(&text(100, "w"), false);
        joined.push("x", false);
        joined.push("y", true);
        let mut split = MinHasher::default();
        split.push(&text(100, "w"), false);
        split.push("x", true);
        split.push("y", true);
        assert_ne!(joined.finish(), split.finish(), "«xy» is one word, «x» and «y» are two");
    }

    #[test]
    fn the_estimate_follows_the_real_similarity() {
        // 1,000 words with the last 100 replaced: 897 shared shingles out of 1,101 distinct,
        // a Jaccard similarity of 0.81.
        let a = text(1000, "w");
        let b = format!("{} {}", text(900, "w"), text(100, "z"));
        let (sa, sb) = (signature(&a).unwrap_or_default(), signature(&b).unwrap_or_default());
        let estimate = agreement(&sa, &sb);
        assert!((0.71..=0.91).contains(&estimate), "estimate {estimate}");
    }

    /// Exact Jaccard similarity of the two texts' shingle sets, the quantity being estimated.
    fn jaccard(a: &str, b: &str) -> f64 {
        use std::collections::HashSet;
        fn shingles(t: &str) -> HashSet<(String, String, String)> {
            let w: Vec<String> = t.split_whitespace().map(str::to_string).collect();
            w.windows(3).map(|s| (s[0].clone(), s[1].clone(), s[2].clone())).collect()
        }
        let (a, b) = (shingles(a), shingles(b));
        a.intersection(&b).count() as f64 / a.union(&b).count() as f64
    }

    #[test]
    fn the_estimator_is_unbiased_across_many_pairs() {
        // 200 pairs of 600-word texts that share a known share of their words. The b-bit
        // estimate, corrected for the 1-in-256 chance agreement as the rule does, must centre on
        // the true similarity and spread as the theory says: sqrt(J(1-J)/128), 0.027 at 0.9.
        // A biased or correlated set of permutations shows up here as a shifted mean or a
        // spread well above that.
        for shared in [300, 540, 570] {
            let (mut errors, mut truth) = (Vec::new(), 0.0);
            for pair in 0..200 {
                let common = text(shared, &format!("c{pair}x"));
                let a = format!("{common} {}", text(600 - shared, &format!("a{pair}x")));
                let b = format!("{common} {}", text(600 - shared, &format!("b{pair}x")));
                truth = jaccard(&a, &b);
                let (sa, sb) = (signature(&a).unwrap_or_default(), signature(&b).unwrap_or_default());
                let estimate = (agreement(&sa, &sb) - 1.0 / 256.0) / (1.0 - 1.0 / 256.0);
                errors.push(estimate - truth);
            }
            let n = errors.len() as f64;
            let mean = errors.iter().sum::<f64>() / n;
            let sd = (errors.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / n).sqrt();
            let expected = (truth * (1.0 - truth) / PERMUTATIONS as f64).sqrt();
            assert!(mean.abs() < 0.01, "J {truth:.3}: mean error {mean:.4}");
            assert!(sd < expected * 1.3, "J {truth:.3}: spread {sd:.4}, theory {expected:.4}");
        }
    }

    #[test]
    fn unrelated_texts_barely_agree() {
        let (a, b) = (
            signature(&text(1000, "a")).unwrap_or_default(),
            signature(&text(1000, "b")).unwrap_or_default(),
        );
        // By chance, one position in 256.
        assert!(agreement(&a, &b) < 0.05, "agreement {}", agreement(&a, &b));
    }
}
