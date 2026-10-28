//! Boyer-Moore 匹配器（坏字符 + 好后缀双启发式），逐行移植 C# BoyerMooreMatcher.cs
//! 语义对齐点：
//! - 坏字符表初始化为 m（不是 -1），填充 m-1-i（最后一次出现优先）
//! - 命中后前进 goodSuffixShifts[0]（保证不漏不重）
//! - 失配时前进 Max(goodSuffixShifts[j], badCharShifts[text[s+j]] - (m-1) + j)

/// 返回 pattern 在 text 中的所有匹配起始下标（升序）
pub fn match_all(text: &[u8], pattern: &[u8]) -> Vec<usize> {
    let n = text.len();
    let m = pattern.len();
    if m == 0 || n < m {
        return Vec::new();
    }

    let bad_char_shifts = build_bad_character_table(pattern);
    let good_suffix_shifts = build_good_suffix_table(pattern);

    let mut result = Vec::new();
    let mut s = 0usize;
    while s <= n - m {
        let mut j = m as isize - 1;
        while j >= 0 && pattern[j as usize] == text[s + j as usize] {
            j -= 1;
        }
        if j < 0 {
            result.push(s);
            s += good_suffix_shifts[0];
        } else {
            let j = j as usize;
            let bad = bad_char_shifts[text[s + j] as usize] as isize;
            let shift_bad = bad - (m as isize - 1) + (j as isize);
            let shift = (good_suffix_shifts[j] as isize).max(shift_bad);
            // 好后缀表恒 >=1，但坏字符组合可能为 0/负，钳到 1 防死循环
            s += shift.max(1) as usize;
        }
    }
    result
}

fn build_bad_character_table(pattern: &[u8]) -> [usize; 256] {
    let m = pattern.len();
    let mut table = [m; 256];
    for (i, &b) in pattern.iter().enumerate() {
        table[b as usize] = m - 1 - i;
    }
    table
}

fn build_good_suffix_table(pattern: &[u8]) -> Vec<usize> {
    let m = pattern.len();
    let mut good_suffix_shifts = vec![m; m];
    let suffix_length_array = suffix_length(pattern);

    let mut j = 0usize;
    // i 从 m-1 递减到 -1（C# int 算术：i=-1 时 m-1-i = m，故用 isize）
    let mut i = m as isize - 1;
    while i >= -1 {
        if i == -1 || suffix_length_array[i as usize] == i as usize + 1 {
            let limit = (m as isize - 1 - i) as usize; // i=-1 时 = m
            while j < limit {
                if good_suffix_shifts[j] == m {
                    good_suffix_shifts[j] = limit;
                }
                j += 1;
            }
        }
        i -= 1;
    }

    for i in 0..m.saturating_sub(1) {
        good_suffix_shifts[m - 1 - suffix_length_array[i]] = m - 1 - i;
    }
    good_suffix_shifts
}

/// Z 算法风格的后缀长度数组（移植 GetSuffixLengthArray）
fn suffix_length(pattern: &[u8]) -> Vec<usize> {
    let m = pattern.len();
    let mut sfx = vec![0usize; m];
    if m == 0 {
        return sfx;
    }
    sfx[m - 1] = m;

    let mut f = 0usize;
    // C#: g = m-1; for i = m-2 downto 0（g 可能为负，用 isize）
    let mut g_i = m as isize - 1;
    let mut i = m as isize - 2;
    while i >= 0 {
        let iu = i as usize;
        if i > g_i && sfx[(iu + m - 1 - f) as usize] < (i - g_i) as usize {
            sfx[iu] = sfx[iu + m - 1 - f];
        } else {
            if i < g_i {
                g_i = i;
            }
            f = iu;
            // 找前导字符不同的后缀
            while g_i >= 0 && pattern[g_i as usize] == pattern[(g_i + m as isize - 1 - f as isize) as usize] {
                g_i -= 1;
            }
            sfx[iu] = (f as isize - g_i) as usize;
        }
        i -= 1;
    }
    sfx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(text: &str, pattern: &str) -> Vec<usize> {
        match_all(text.as_bytes(), pattern.as_bytes())
    }

    #[test]
    fn basic_matches() {
        assert_eq!(matches("abc", "abc"), vec![0]);
        assert_eq!(matches("abc", "d"), Vec::<usize>::new());
        assert_eq!(matches("aabc", "abc"), vec![1]);
    }

    #[test]
    fn all_occurrences() {
        // 与 String::match_indices 全量对拍
        let text = "GCATCGCAGAGAGTATACAGTACG".repeat(10);
        let pattern = "GCAGAGAG";
        let expected: Vec<usize> = text
            .match_indices(pattern)
            .map(|(i, _)| i)
            .collect();
        assert!(!expected.is_empty());
        assert_eq!(matches(&text, pattern), expected);
    }

    #[test]
    fn random_against_naive() {
        // 伪随机（固定种子序列）对拍朴素算法
        let mut seed = 0x12345678u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..200 {
            let n = (next() % 500 + 10) as usize;
            let m = (next() % 8 + 1) as usize;
            let text: Vec<u8> = (0..n).map(|_| (next() % 4) as u8).collect(); // 小字母表，高碰撞
            let pattern: Vec<u8> = (0..m).map(|_| (next() % 4) as u8).collect();
            let expected: Vec<usize> = (0..=n.saturating_sub(m))
                .filter(|&i| text[i..i + m] == pattern[..])
                .collect();
            assert_eq!(match_all(&text, &pattern), expected, "text={:?} pattern={:?}", text, pattern);
        }
    }

    #[test]
    fn pattern_longer_than_text() {
        assert!(matches("ab", "abc").is_empty());
    }

    #[test]
    fn good_suffix_termination() {
        // 后缀回退场景，确保不死循环
        assert_eq!(matches("aaaaaaaa", "aa"), vec![0, 1, 2, 3, 4, 5, 6]);
    }
}