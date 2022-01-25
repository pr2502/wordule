use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;
use std::mem;


#[derive(Clone, Default)]
struct LetterSet {
    set: u32,
}

fn to_ascii_index(letter: char) -> u8 {
    assert!(letter.is_ascii_lowercase(), "unsupported character");
    (letter as u8) - b'a'
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
        self.set |= 1u32 << to_ascii_index(letter);
    }

    fn contains(&self, letter: char) -> bool {
        (self.set & (1u32 << to_ascii_index(letter))) != 0
    }
}


#[derive(Default)]
struct LetterCount {
    map: [usize; 26],
}

impl LetterCount {
    fn increment(&mut self, letter: char) {
        self.map[usize::from(to_ascii_index(letter))] += 1;
    }

    fn get(&self, letter: char) -> usize {
        self.map[usize::from(to_ascii_index(letter))]
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
            for ch in 'a'..='z' {
                if word_set.contains(ch) {
                    count.increment(ch);
                }
            }
        }
        Scoring { count, half }
    }

    fn letter_score(&self, letter: char) -> isize {
        self.half - isize::abs(self.count.get(letter) as isize - self.half)
    }

    /// scores a word by summing up scores for its unique letters, each letter is scored higher the
    /// closer it is to being in half of the present counted words
    fn word_score(&self, word: &str) -> isize {
        let mut total = 0;
        let set = LetterSet::from_word(word);
        for ch in 'a'..='z' {
            if set.contains(ch) {
                total += self.letter_score(ch);
            }
        }
        total
    }
}


fn main() -> Result<()> {
    let mut file = File::open("/usr/share/dict/words").context("opening words file")?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).context("reading words file")?;

    // parse all possibly applicable words from the file
    let words = buf.lines()
        .filter(|line| line.chars().all(|ch| ch.is_ascii_lowercase()))
        .collect::<Vec<_>>();

    eprintln!("wordule: wordle solving thingy");
    eprintln!("1. enter the game word length");
    eprintln!("2. enter a loop of guesses:");
    eprintln!("  2a. pick a word from the top 10 words and write the picked word");
    eprintln!("  2b. tell wordule what the answer was, for each letter in the guessed word write:");
    eprintln!("    `x` for grey (no match in word)");
    eprintln!("    `?` for orange (match somewhere in the word)");
    eprintln!("    `o` for green (exact match)");
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
    let mut forbidden_position = vec![LetterSet::default(); length]; // letters which are only forbidden for a certain position
    let mut forbidden_everywhere = LetterSet::default(); // letters that can never be used again

    // guess loop
    loop {
        // sort current possible guesses
        let score = Scoring::new(&words);
        words.sort_unstable_by_key(|word| -score.word_score(word));

        let guesses = if words.len() < 10 { &words[..] } else { &words[..10] };
        println!("guesses:");
        for guess in guesses {
            println!("  {guess}   {}", score.word_score(guess));
        }

        // pick word
        let picked = loop {
            match rl.readline("picked> ").context("readline")? {
                word if word.len() == length => break word,
                _ => eprintln!("length doesn't match"),
            }
        };

        // parse wordle response
        loop {
            let res = rl.readline("response> ").context("readline")?;
            if res.len() != length {
                eprintln!("response length doesn't match");
                continue
            }

            for (i, (res, pick)) in res.chars().zip(picked.chars()).enumerate() {
                match res {
                    'x' => {
                        forbidden_everywhere.insert(pick);
                    },
                    '?' => {
                        forbidden_position[i].insert(pick);
                    },
                    'o' => {
                        fixed_letters[i] = Some(pick);
                    },
                    _ => {
                        eprintln!("invalid response syntax at {i}: `{res}`");
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
                    if forbidden_everywhere.contains(ch) {
                        return false
                    }
                    // mismatched fixed letter
                    if let Some(fixed) = &fixed_letters[i] {
                        if *fixed != ch {
                            return false
                        }
                    }
                    // forbidden positional letter
                    if forbidden_position[i].contains(ch) {
                        return false
                    }
                }

                true
            })
            .collect();
    }
}
