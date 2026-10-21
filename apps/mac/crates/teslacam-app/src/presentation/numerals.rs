//! Splitting text into numeral runs (set in Barlow) and the rest (the UI font): `2分` → `2` + `分`.
//!
//! Same rule as the web's `NumeralText`: a run starts with an ASCII digit and continues over digits
//! and `: , . / –`.

/// A piece of text and whether it is a numeral run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run<'a> {
    /// The text of the run.
    pub text: &'a str,
    /// Whether it is set in the numeral font.
    pub numeral: bool,
}

fn continues_numeral(character: char) -> bool {
    character.is_ascii_digit() || matches!(character, ':' | ',' | '.' | '/' | '\u{2013}')
}

/// The runs of `text` in order; empty runs are dropped.
pub fn numeral_runs(text: &str) -> Vec<Run<'_>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut numeral = false;
    for (index, character) in text.char_indices() {
        let next = if numeral {
            continues_numeral(character)
        } else {
            character.is_ascii_digit()
        };
        if next != numeral {
            if index > start {
                runs.push(Run {
                    text: &text[start..index],
                    numeral,
                });
            }
            start = index;
            numeral = next;
        }
    }
    if text.len() > start {
        runs.push(Run {
            text: &text[start..],
            numeral,
        });
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(text: &str) -> Vec<(&str, bool)> {
        numeral_runs(text)
            .into_iter()
            .map(|run| (run.text, run.numeral))
            .collect()
    }

    #[test]
    fn digits_and_units_alternate() {
        assert_eq!(runs("2分"), [("2", true), ("分", false)]);
        assert_eq!(runs("6カメラ"), [("6", true), ("カメラ", false)]);
        assert_eq!(
            runs("1時間10分"),
            [("1", true), ("時間", false), ("10", true), ("分", false)]
        );
    }

    #[test]
    fn times_ranges_and_grouped_counts_stay_whole() {
        assert_eq!(
            runs("20:35:30に検知"),
            [("20:35:30", true), ("に検知", false)]
        );
        assert_eq!(runs("20:35\u{2013}20:45"), [("20:35\u{2013}20:45", true)]);
        assert_eq!(
            runs("1,250 / 2,000"),
            [("1,250", true), (" / ", false), ("2,000", true)]
        );
        assert_eq!(runs("8/20"), [("8/20", true)]);
    }

    #[test]
    fn text_without_digits_is_one_run() {
        assert_eq!(runs("記録がありません"), [("記録がありません", false)]);
        assert!(runs("").is_empty());
        assert_eq!(
            runs("2026-08-20"),
            [
                ("2026", true),
                ("-", false),
                ("08", true),
                ("-", false),
                ("20", true)
            ]
        );
    }
}
