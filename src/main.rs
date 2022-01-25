use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;
use regex::Regex;
use std::mem;
use std::collections::HashSet;


/// all scoring assumes that words are only comprised of [a-z] ascii characters
#[derive(Debug)]
struct Scoring {
    /// half the number of total words
    half: isize,
    /// number of words a letter occurs in
    letters: [isize; 26],
}

fn word_letters(word: &str) -> [u8; 26] {
    let mut letters = [0; 26];
    for ch in word.as_bytes() {
        letters[usize::from(ch - b'a')] += 1;
    }
    letters
}

impl Scoring {
    fn new(words: &[&str]) -> Scoring {
        let mut letters = [0; 26];
        let half = (words.len() as isize) / 2;
        for word in words {
            for (i, &cnt) in word_letters(word).iter().enumerate() {
                letters[i] += isize::min(1, cnt.into());
            }
        }
        words.iter()
            .flat_map(|w| w.as_bytes())
            .for_each(|ch| letters[usize::from(ch - b'a')] += 1);
        Scoring { letters, half }
    }

    /// scores a word by summing up scores for its unique letters, each letter is scored higher the
    /// closer it is to being in half of the present counted words
    fn score(&self, word: &str) -> isize {
        word_letters(word)
            .iter()
            .zip(&self.letters)
            .map(|(&cnt, &freq)| {
                // FIXME this is probably wrong :(
                if cnt == 0 { return 0 }
                100_000 - isize::abs(self.half - freq)
            })
            .sum()
    }
}


fn main() -> Result<()> {
    let mut file = File::open("/usr/share/dict/words").context("opening words file")?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).context("reading words file")?;

    // parse all possibly applicable words from the file
    let words_re = Regex::new("(?m)^[a-z]+$").unwrap();
    let words = words_re
        .find_iter(&buf)
        .map(|m| m.as_str())
        .collect::<Vec<_>>();

    eprintln!("wurdle: wordle solving thingy");
    eprintln!("1. enter the game word length");
    eprintln!("2. enter a loop of guesses:");
    eprintln!("  2a. pick a word from the top 10 words and write the picked word");
    eprintln!("  2b. tell wurdle what the answer was, for each letter in the guessed word write:");
    eprintln!("    `_` for grey (no match in word)");
    eprintln!("    `?` for orange (match somewhere in the word)");
    eprintln!("    `x` for green (exact match)");
    let mut rl = rustyline::Editor::<()>::new();

    let length = loop {
        match rl.readline("word length> ").context("readline")?.parse::<usize>() {
            Ok(length) if length > 0 => break length,
            _ => eprintln!("must be a positive number"),
        }
    };

    // filter by length
    let mut words = words.into_iter()
        .filter(|word| word.len() == length)
        .collect::<Vec<_>>();

    // hints
    let mut fixed_letters = vec![None; length]; // letters we already know for sure
    let mut forbidden_position = vec![HashSet::new(); length]; // letters which are only forbidden for a certain position
    let mut forbidden_everywhere = [false; 26]; // letters that can never be used again

    // guess loop
    loop {
        // sort current possible guesses
        let score = Scoring::new(&words);
        words.sort_unstable_by_key(|word| -score.score(word));

        let guesses = if words.len() < 10 { &words[..] } else { &words[..10] };
        println!("guesses:");
        for guess in guesses {
            println!("  {guess}   {}", score.score(guess));
        }

        // pick word
        let picked = loop {
            match rl.readline("picked> ").context("readline")? {
                word if word.len() == length => break word,
                _ => eprintln!("length doesn't match"),
            }
        };
        let picked = picked.as_bytes();

        // parse wordle response
        loop {
            let res = rl.readline("response> ").context("readline")?;
            if res.len() != length {
                eprintln!("response length doesn't match");
                continue
            }

            for (i, ch) in res.chars().enumerate() {
                match ch {
                    '_' => {
                        forbidden_everywhere[usize::from(picked[i] - b'a')] = true;
                    },
                    '?' => {
                        forbidden_position[i].insert(picked[i] as char);
                    },
                    'x' => {
                        fixed_letters[i] = Some(picked[i] as char);
                    },
                    _ => {
                        eprintln!("invalid response syntax at {i}: `{ch}`");
                        continue
                    }
                }
            }
            break
        };

        // filter out impossible guesses
        words = mem::take(&mut words).into_iter()
            .filter(|word| {
                for (i, ch) in word.chars().enumerate() {
                    // forbidden letter
                    if forbidden_everywhere[usize::from(ch as u8 - b'a')] {
                        return false
                    }
                    // mismatched fixed letter
                    if let Some(fixed) = &fixed_letters[i] {
                        if *fixed != ch {
                            return false
                        }
                    }
                    // forbidden positional letter
                    if forbidden_position[i].contains(&ch) {
                        return false
                    }
                }

                true
            })
            .collect();
    }
}
