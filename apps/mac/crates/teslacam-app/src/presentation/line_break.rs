//! Line breaking for Japanese UI copy (`DESIGN.md` §1/§8: readable copy): GPUI breaks CJK text
//! between any two characters, which strands one-character tails (`…ありませ` / `ん`) and starts
//! lines with `、` or `。`. This breaker keeps ASCII words whole, never starts a line with closing
//! punctuation or a small kana, and pulls at least [`MIN_TAIL_CHARS`] characters onto the last line.

/// The fewest characters the last line of a wrapped paragraph keeps.
pub const MIN_TAIL_CHARS: usize = 3;

/// Characters that must not begin a line (kinsoku shori, line-start prohibited).
fn is_no_start(c: char) -> bool {
    "、。，．・：；？！）」』】〕｝〉》ー…‥ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ),.:;!?%]}"
        .contains(c)
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "._-/:'@#&+".contains(c)
}

/// Splits `text` into the pieces a line may break between: an ASCII word (with the spaces after
/// it) stays whole; every other character is its own piece.
fn units(text: &str) -> Vec<&str> {
    let mut units = Vec::new();
    let mut start = None;
    for (index, c) in text.char_indices() {
        if is_word_char(c) {
            start.get_or_insert(index);
            continue;
        }
        if c == ' ' && start.is_some() {
            continue;
        }
        if let Some(word) = start.take() {
            units.push(&text[word..index]);
        }
        units.push(&text[index..index + c.len_utf8()]);
    }
    if let Some(word) = start {
        units.push(&text[word..]);
    }
    units
}

fn starts_badly(unit: &str) -> bool {
    unit.chars().next().is_some_and(is_no_start)
}

fn chars(line: &[&str]) -> usize {
    line.iter().map(|unit| unit.chars().count()).sum()
}

/// Breaks one paragraph (no `\n`) into lines at most `max_width` wide, where `measure` gives the
/// width of a string.
fn break_paragraph(text: &str, max_width: f32, measure: &impl Fn(&str) -> f32) -> Vec<String> {
    let fits = |line: &[&str]| measure(line.concat().trim_end()) <= max_width + 0.01;
    let units = units(text);
    if units.is_empty() || fits(&units) {
        return vec![text.to_owned()];
    }
    let mut lines: Vec<Vec<&str>> = vec![Vec::new()];
    for unit in units {
        let current = lines.last_mut().expect("a line");
        current.push(unit);
        if current.len() == 1 || fits(current) {
            continue;
        }
        current.pop();
        // Closing punctuation stays with the character before it.
        let mut carried = Vec::new();
        while starts_badly(unit) && current.len() > 1 && carried.is_empty() {
            carried.push(current.pop().expect("a unit"));
        }
        carried.push(unit);
        lines.push(carried);
    }
    // No stranded tail: pull characters down while the last line stays within the width.
    while lines.len() > 1 && chars(&lines[lines.len() - 1]) < MIN_TAIL_CHARS {
        let previous = lines.len() - 2;
        if lines[previous].len() < 2 {
            break;
        }
        let moved = *lines[previous].last().expect("a unit");
        let mut last = vec![moved];
        last.extend_from_slice(&lines[previous + 1]);
        let remaining = &lines[previous][..lines[previous].len() - 1];
        if starts_badly(moved)
            || !fits(&last)
            || starts_badly(remaining.last().copied().unwrap_or(""))
        {
            break;
        }
        lines[previous].pop();
        lines[previous + 1] = last;
    }
    lines
        .into_iter()
        .map(|line| line.concat().trim_end().to_owned())
        .collect()
}

/// Breaks `text` into lines at most `max_width` wide (explicit `\n` breaks are kept).
pub fn break_lines(text: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    text.split('\n')
        .flat_map(|paragraph| break_paragraph(paragraph, max_width, &measure))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character is 1 wide.
    fn width(text: &str) -> f32 {
        text.chars().count() as f32
    }

    #[test]
    fn short_text_stays_on_one_line() {
        assert_eq!(
            break_lines("記録がありません", 20.0, width),
            ["記録がありません"]
        );
    }

    #[test]
    fn the_last_line_keeps_at_least_three_characters() {
        // Greedy: `左リピーターの記録がありませ` / `ん`.
        assert_eq!(
            break_lines("左リピーターの記録がありません", 14.0, width),
            ["左リピーターの記録があり", "ません"]
        );
        assert_eq!(
            break_lines("TeslaCamフォルダを確認できませんでした。", 21.0, width),
            ["TeslaCamフォルダを確認できませんで", "した。"]
        );
    }

    #[test]
    fn lines_never_start_with_closing_punctuation_or_small_kana() {
        for lines in [
            break_lines("リア、右リピーターの記録がありません", 2.0 + 1.0, width),
            break_lines("フォルダの読み込みが許可されませんでした。", 9.0, width),
            break_lines(
                "ファイルを確認中です。もう一度お試しください。",
                10.0,
                width,
            ),
        ] {
            for line in &lines[1..] {
                assert!(
                    !line.chars().next().is_some_and(is_no_start),
                    "{lines:?} starts a line with {line}"
                );
            }
        }
    }

    #[test]
    fn ascii_words_stay_whole() {
        let lines = break_lines("TeslaCamフォルダを選んでください", 10.0, width);
        assert_eq!(lines[0], "TeslaCamフォ");
        assert!(lines.iter().all(|line| !line.ends_with("Tesla")));
    }

    #[test]
    fn explicit_breaks_are_kept() {
        assert_eq!(
            break_lines(
                "再生できるカメラ映像がありません。\n読み込み後に再試行してください。",
                40.0,
                width
            ),
            [
                "再生できるカメラ映像がありません。",
                "読み込み後に再試行してください。"
            ]
        );
    }

    #[test]
    fn every_line_fits_unless_a_single_piece_is_wider() {
        let text = "再生可能な動画がありません。TeslaCamのフォルダ構成を確認してください。";
        for max in 4..30 {
            let lines = break_lines(text, max as f32, width);
            assert_eq!(lines.concat(), text, "no characters lost at {max}");
            for line in &lines {
                assert!(
                    width(line) <= max as f32 || units(line).len() == 1,
                    "{line} wider than {max}"
                );
            }
        }
    }
}
