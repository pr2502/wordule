use anyhow::{ensure, Context, Result};
use clap::Parser;
use std::cmp::Ordering;
use std::fs::File;
use std::io::Read;
use std::mem;

#[derive(Clone, Default)]
struct LetterSet {
    set: u32,
}

fn to_letter_index(letter: char) -> Option<u8> {
    letter.is_ascii_lowercase().then(|| (letter as u8) - b'a')
}

fn letters() -> impl Iterator<Item = char> {
    'a'..='z'
}

impl LetterSet {
    fn from_word(word: &str) -> LetterSet {
        let mut set = LetterSet::default();
        for ch in word.chars() {
            set.insert(ch);
        }
        set
    }

    fn insert(&mut self, letter: char) {
        if let Some(index) = to_letter_index(letter) {
            self.set |= 1u32 << index;
        }
    }

    fn contains(&self, letter: char) -> bool {
        if let Some(index) = to_letter_index(letter) {
            (self.set & (1u32 << index)) != 0
        } else {
            false
        }
    }
}

#[derive(Default)]
struct LetterCount {
    map: [usize; 26],
}

impl LetterCount {
    fn increment(&mut self, letter: char) {
        if let Some(index) = to_letter_index(letter) {
            self.map[usize::from(index)] += 1;
        }
    }

    fn get(&self, letter: char) -> usize {
        if let Some(index) = to_letter_index(letter) {
            self.map[usize::from(index)]
        } else {
            0
        }
    }
}

/// all scoring assumes that words are only comprised of [a-z] ascii characters
struct Scoring {
    /// half the number of total words
    half: isize,
    /// number of words a letter occurs in
    count: LetterCount,
}

impl Scoring {
    fn new(words: &[&str]) -> Scoring {
        let mut count = LetterCount::default();
        let half = (words.len() as isize) / 2;
        for word in words {
            let word_set = LetterSet::from_word(word);
            for ch in letters() {
                if word_set.contains(ch) {
                    count.increment(ch);
                }
            }
        }
        Scoring { count, half }
    }

    fn max_score(&self) -> isize {
        self.half
    }

    fn letter_score(&self, letter: char) -> f32 {
        let abs_score = self.half - isize::abs(self.count.get(letter) as isize - self.half);
        (abs_score as f32) / (self.half as f32)
    }

    /// scores a word by summing up scores for its unique letters, each letter is scored higher the
    /// closer it is to being in half of the present counted words
    fn word_score(&self, word: &str, present_letters: &LetterSet, early: bool) -> f32 {
        let mut total = 0.0;
        let set = LetterSet::from_word(word);
        for ch in letters() {
            if set.contains(ch) {
                let score = self.letter_score(ch);
                // adjust score for early and late game, in early game letters which we haven't
                // tried yet get a boost, in late game letters which are definitely included get a
                // boost. it's up to the player to pick using the early/late game sorting
                let adjust = match (early, present_letters.contains(ch)) {
                    // early game
                    (true, true) => 0.0,    // do not guess already known letters
                    (true, false) => score, // leave the not-present letters alone

                    // late game
                    (false, true) => score * 2.0, // buff the present letters
                    (false, false) => score,      // leave the non-present letters alone
                };
                total += adjust;
            }
        }
        total
    }
}

#[derive(Parser)]
struct Args {
    /// Guessed word length
    #[clap(long, default_value = "5")]
    length: usize,

    /// Path to a dictionary file
    #[clap(long, default_value = "/usr/share/dict/words")]
    dict: String,

    /// Amount of best guesses to show
    #[clap(long, default_value = "10")]
    guesses: usize,

    /// If present prints score for the word and exit
    #[clap(long)]
    score_word: Option<String>,

    /// Show letter scores
    #[clap(long)]
    letter_scores: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut file = File::open(&args.dict).context("opening words file")?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .context("reading words file")?;

    let length = args.length;
    ensure!(length > 0, "word length must be positive");

    // parse all possibly applicable words from the file
    let mut all_words = buf
        .lines()
        .filter(|line| line.len() == length && line.chars().all(|ch| ch.is_ascii_lowercase()))
        .collect::<Vec<_>>();

    if let Some(score_word) = &args.score_word {
        let score = Scoring::new(&all_words);

        println!(
            "  {score_word}      {}",
            // with an empty set there is no difference between early/late scores
            score.word_score(score_word, &LetterSet::default(), true),
        );
        return Ok(());
    }

    eprintln!(
        "\
wordule: wordle solving thingy
    1. pick a word from the top 10 words and write the picked word
    2. tell wordule what the answer was, for each letter in the guessed word write:
        `x` for grey (no match in word)
        `?` for orange (match somewhere in the word)
        `o` for green (exact match)
    3. repeat
"
    );

    let mut words = all_words.clone();
    let mut rl = rustyline::Editor::<()>::new();

    // hints
    let mut fixed_letters = vec![None; length]; // letters we already know for sure
    let mut fixed_anywhere = LetterSet::default(); // letters which have ben used for a position fix
    let mut forbidden_position = vec![LetterSet::default(); length]; // letters which are only forbidden for a certain position
    let mut forbidden_everywhere = LetterSet::default(); // letters that can never be used again
    let mut present_everywhere = LetterSet::default(); // letters which are definitely present

    // guess loop
    loop {
        // sort current possible guesses
        let score = Scoring::new(&words);
        all_words.sort_by(|left, right| {
            let ls = score.word_score(left, &present_everywhere, true);
            let rs = score.word_score(right, &present_everywhere, true);
            ls.partial_cmp(&rs).unwrap_or(Ordering::Equal).reverse()
        });
        let early_guesses = Vec::from(if all_words.len() < args.guesses {
            &all_words[..]
        } else {
            &all_words[..args.guesses]
        });
        words.sort_by(|left, right| {
            let ls = score.word_score(left, &present_everywhere, true);
            let rs = score.word_score(right, &present_everywhere, true);
            ls.partial_cmp(&rs).unwrap_or(Ordering::Equal).reverse()
        });
        let late_guesses = Vec::from(if words.len() < args.guesses {
            &words[..]
        } else {
            &words[..args.guesses]
        });

        if args.letter_scores {
            let mut scores = letters()
                .map(|ch| (ch, score.letter_score(ch)))
                .collect::<Vec<_>>();
            scores.sort_by(|(_, ls), (_, rs)| {
                ls.partial_cmp(rs).unwrap_or(Ordering::Equal).reverse()
            });

            println!("maximum {}", score.max_score());
            for (ch, score) in &scores[..13] {
                print!("  {ch} {score:>4.3}");
            }
            println!();
            for (ch, score) in &scores[13..] {
                print!("  {ch} {score:>4.3}");
            }
            println!();
        }

        println!("guesses (early, late):");
        for (early, late) in early_guesses.iter().zip(&late_guesses) {
            println!(
                "{early: >7}  {es: >4.2}  {late: >7}  {ls: >4.2}",
                es = score.word_score(early, &present_everywhere, true),
                ls = score.word_score(late, &present_everywhere, false),
            );
        }

        // pick word
        let picked = loop {
            let word = rl.readline("picked> ").context("readline")?;
            if word.len() != length {
                eprintln!("length doesn't match");
                continue;
            }
            if word.chars().any(|ch| !ch.is_ascii_lowercase()) {
                eprintln!("contains invalid chars");
                continue;
            }
            if word.chars().all(|ch| ['o', 'x'].contains(&ch)) {
                eprintln!("only contains `o` and `x`, we want picked guess not pattern");
                continue;
            }
            break word;
        };

        // parse wordle response
        loop {
            let res = rl.readline("response> ").context("readline")?;
            if res.len() != length {
                eprintln!("response length doesn't match");
                continue;
            }

            for (i, (res, pick)) in res.chars().zip(picked.chars()).enumerate() {
                match res {
                    'x' => {
                        if fixed_anywhere.contains(pick) {
                            // the letter was already used somewhere as a fix but we didn't get `?`
                            // for a different position, make it forbidden everywhere but the fixed
                            // position
                            for i in 0..length {
                                if let Some(fix) = fixed_letters[i] {
                                    if fix != pick {
                                        forbidden_position[i].insert(pick);
                                    }
                                }
                            }
                        } else {
                            // it was never used
                            forbidden_everywhere.insert(pick);
                        }
                    }
                    '?' => {
                        forbidden_position[i].insert(pick);
                        present_everywhere.insert(pick);
                    }
                    'o' => {
                        fixed_letters[i] = Some(pick);
                        fixed_anywhere.insert(pick);
                        present_everywhere.insert(pick);
                    }
                    _ => {
                        eprintln!("invalid response syntax at {i}: `{res}`");
                        continue;
                    }
                }
            }
            break;
        }

        // filter out impossible guesses
        words = mem::take(&mut words)
            .into_iter()
            .filter(|word| {
                for (i, ch) in word.chars().enumerate() {
                    // forbidden letter
                    if forbidden_everywhere.contains(ch) {
                        return false;
                    }
                    // mismatched fixed letter
                    if let Some(fixed) = &fixed_letters[i] {
                        if *fixed != ch {
                            return false;
                        }
                    }
                    // forbidden positional letter
                    if forbidden_position[i].contains(ch) {
                        return false;
                    }
                }

                true
            })
            .collect();
    }
}
