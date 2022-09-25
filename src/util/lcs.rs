use unicode_segmentation::UnicodeSegmentation;

use super::array2d::Array2d;

/// Find the size of the longest common subsequence of unicode graphemes of two strings
pub fn lcs(s1: &str, s2: &str) -> usize {
    let graphemes = |s| UnicodeSegmentation::graphemes(s, true).collect::<Vec<_>>();
    let s1_graphemes = graphemes(s1);
    let s2_graphemes = graphemes(s2);

    // Make sure s1 is the longer string
    let mut s1_graphemes = &s1_graphemes;
    let mut s2_graphemes = &s2_graphemes;
    if s1_graphemes.len() < s2_graphemes.len() {
        std::mem::swap(&mut s1_graphemes, &mut s2_graphemes);
    }

    // Trim graphemes
    let (s1_graphemes, s2_graphemes, trim_count) = trim(s1_graphemes, s2_graphemes);

    // DP LCS algorithm
    let mut arr: Array2d<usize> = Array2d::new(2, 1 + s2_graphemes.len());
    for i in 0..s1_graphemes.len() {
        for j in 0..s2_graphemes.len() {
            arr[[1 - i % 2, j + 1]] = if s1_graphemes[i] == s2_graphemes[j] {
                1 + arr[[i % 2, j]]
            } else {
                std::cmp::max(arr[[1 - i % 2, j]], arr[[i % 2, j + 1]])
            };
        }
    }

    arr[[s1_graphemes.len() % 2, s2_graphemes.len()]] + trim_count
}

/// Trim beginning and end of graphemes if they match in both strings, also return the number of
/// trims performed
fn trim<'a, 'b, 'c, 'd>(s1: &'a [&'b str], s2: &'c [&'d str]) -> (&'a [&'b str], &'c [&'d str], usize) {
    let mut s1 = s1;
    let mut s2 = s2;
    let mut trim_count = 0;

    // Trim beginning
    while !s1.is_empty() && !s2.is_empty() && s1[0] == s2[0] {
        s1 = &s1[1..];
        s2 = &s2[1..];
        trim_count += 1;
    }

    // Trim end
    while !s1.is_empty() && !s2.is_empty() && s1[s1.len() - 1] == s2[s2.len() - 1] {
        s1 = &s1[..s1.len() - 1];
        s2 = &s2[..s2.len() - 1];
        trim_count += 1;
    }

    (s1, s2, trim_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        assert_eq!(3, lcs("abcde", "ace"));
        assert_eq!(3, lcs("ace", "abcde"));
        assert_eq!(3, lcs("abc", "abc"));
        assert_eq!(0, lcs("abc", "def"));
    }

    #[test]
    fn test_empty() {
        assert_eq!(0, lcs("", ""));
        assert_eq!(0, lcs("", "abc"));
        assert_eq!(0, lcs("abc", ""));
    }

    #[test]
    fn test_trim() {
        assert_eq!(3, lcs("abcdef", "abcxyz"));
        assert_eq!(3, lcs("abcdef", "xyzdef"));
        assert_eq!(4, lcs("abcdef", "abxyef"));
    }

    #[test]
    fn test_unicode() {
        let s1 = "풍부하게 바이며, 이는 심장의 것이다. 아름다우냐?";
        let s2 = "쓸쓸하랴? 못할 노래하며 일월과 갑 아름다우냐?";
        assert_eq!(12, lcs(s1, s2));

        let s1 = "Λορεμ ιπσθμ δολορ σιτ αμετ, ατ μει πορρο αβηορρεαντ.";
        let s2 = "Εθμ ιμπεδιτ ταcιματεσ εθ, αν μθνερε δισσεντιετ αππελλαντθρ vισ.";
        assert_eq!(25, lcs(s1, s2));
    }
}
