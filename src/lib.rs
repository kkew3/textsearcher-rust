use std::fs;
use regex::{Regex, RegexBuilder};
use rayon::prelude::*;
use pyo3::prelude::*;


/// AND of patterns, where each pattern except for the first is the OR of some sub-patterns.
#[pyclass]
pub struct QueryGroup {
    pub patterns: Vec<Regex>,
}

#[pymethods]
impl QueryGroup {
    #[new]
    pub fn new(primary_atom: String, and_of_or_atoms: Vec<Vec<String>>) -> Self {
        let primary_pattern = get_regex_for_atom(&primary_atom);
        let mut patterns = vec![primary_pattern];
        for or_grp in and_of_or_atoms.iter() {
            patterns.push(get_regex_for_atoms(or_grp));
        }

        QueryGroup {
            patterns,
        }
    }
}

fn is_match(query_group: &QueryGroup, path: &str) -> bool {
    match fs::read_to_string(path) {
        Ok(contents) => {
            for pat in query_group.patterns.iter() {
                if !pat.is_match(&contents) {
                    return false;
                }
            }
            true
        }
        Err(_) => false
    }
}

pub fn search_text<'a>(query_group: &QueryGroup, textfile_paths: &'a Vec<String>, parallel: bool) -> Vec<&'a String> {
    if parallel {
        textfile_paths
            .par_iter()
            .filter(|path| is_match(&query_group, path))
            .collect()
    } else {
        textfile_paths
            .iter()
            .filter(|path| is_match(&query_group, path))
            .collect()
    }
}

/// Build regex for atom query.
/// See `test__get_regex_for_atom` for the transform rules.
/// The regex is built so that it's robust to noise brought by pdf-to-text parsing.
fn get_regex_for_atom(atom: &str) -> Regex {
    let regex = _get_regex_for_atom(atom);
    RegexBuilder::new(&regex)
        .multi_line(true)
        .case_insensitive(true)
        .dot_matches_new_line(false)
        .build()
        .unwrap()
}

/// The difference from `get_regex_for_atom` is that this OR the atoms together.
fn get_regex_for_atoms(atoms: &Vec<String>) -> Regex {
    let regexes: Vec<_> = atoms
        .into_iter()
        .map(|a| _get_regex_for_atom(a))
        .collect();
    RegexBuilder::new(&regexes.join("|"))
        .multi_line(true)
        .case_insensitive(true)
        .dot_matches_new_line(false)
        .build()
        .unwrap()
}

#[pyclass]
pub struct FilePaths {
    pub paths: Vec<String>,
}

#[pymethods]
impl FilePaths {
    #[new]
    pub fn new(paths: Vec<String>) -> Self {
        FilePaths {
            paths,
        }
    }
}

#[pyfunction]
#[pyo3(name = "search_text")]
pub fn py_search_text(query_group: &QueryGroup, textfile_paths: &FilePaths) -> Vec<String> {
    let filtered: Vec<_> = search_text(query_group, &textfile_paths.paths, true);
    let mut paths = Vec::new();
    for p in filtered {
        paths.push(String::from(p));
    }
    paths
}

#[pymodule]
#[pyo3(name = "textsearcher")]
fn py_module(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<QueryGroup>()?;
    m.add_class::<FilePaths>()?;
    m.add_function(wrap_pyfunction!(py_search_text, m)?)?;
    Ok(())
}

/// Returns String to make testing convenient
fn _get_regex_for_atom(atom: &str) -> String {
    let mut regex = String::new();
    let mut word = String::new();
    let mut prev_ch = '\u{0}';  // represents the beginning or the ending
    let mut word_commited = false;

    enum CharType {
        /// beginning or ending
        Term,
        /// whitespace characters
        Blank,
        /// CJK characters
        Hans,
        /// e.g. ASCII
        Other,
    }

    let get_char_type = |ch| {
        if ch == '\u{0}' {
            CharType::Term
        } else if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
            CharType::Blank
        } else if (ch >= '\u{4e00}' && ch <= '\u{9fa5}') || (ch >= '\u{3040}' && ch <= '\u{30ff}') {
            CharType::Hans
        } else {
            CharType::Other
        }
    };

    for ch in atom.chars().chain("\0".chars()) {
        match (get_char_type(prev_ch), get_char_type(ch)) {
            (CharType::Term, CharType::Term) => (),
            (CharType::Term, CharType::Blank) => (),
            (CharType::Term, CharType::Hans) => {
                // push ch to word
                word.push(ch);
                word_commited = false;
                prev_ch = ch;
            }
            (CharType::Term, CharType::Other) => {
                //regex.push_str("(\\b|[\\u4e00-\\u9fa5\\u3040-\\u30FF])");
                // push ch to word
                word.push(ch);
                word_commited = false;
                prev_ch = ch;
            }
            (CharType::Blank, _) => (),
            (CharType::Hans, CharType::Term) => {
                if !word_commited {
                    // commit word to regex
                    regex.push_str(&regex::escape(&word));
                    word.clear();
                    word_commited = true;
                }
            }
            (CharType::Hans, CharType::Blank) => {
                if !word_commited {
                    // commit word to regex
                    regex.push_str(&regex::escape(&word));
                    word.clear();
                    word_commited = true;
                }
            }
            (CharType::Hans, CharType::Hans) => {
                if word_commited {
                    regex.push_str("\\s+");
                } else {
                    // commit word to regex
                    regex.push_str(&regex::escape(&word));
                    word.clear();
                    //word_commited = true;
                    regex.push_str("\\s*");
                }
                // push ch to word
                word.push(ch);
                word_commited = false;
                prev_ch = ch;
            }
            (CharType::Hans, CharType::Other) => {
                if !word_commited {
                    // commit word to regex
                    regex.push_str(&regex::escape(&word));
                    word.clear();
                    //word_commited = true;
                }
                regex.push_str("\\s*");
                // push ch to word
                word.push(ch);
                word_commited = false;
                prev_ch = ch;
            }
            (CharType::Other, CharType::Term) => {
                if !word_commited {
                    // commit word to regex
                    regex.push_str(&regex::escape(&word));
                    word.clear();
                    word_commited = true;
                }
                //regex.push_str("(\\b|[\\u4e00-\\u9fa5\\u3040-\\u30FF])");
                prev_ch = ch;
            }
            (CharType::Other, CharType::Blank) => {
                if !word_commited {
                    // commit word to regex
                    regex.push_str(&regex::escape(&word));
                    word.clear();
                    word_commited = true;
                }
            }
            (CharType::Other, CharType::Hans) => {
                if !word_commited {
                    // commit word to regex
                    regex.push_str(&regex::escape(&word));
                    word.clear();
                    //word_commited = true;
                }
                regex.push_str("\\s*");
                // push ch to word
                word.push(ch);
                word_commited = false;
                prev_ch = ch;
            }
            (CharType::Other, CharType::Other) => {
                if word_commited {
                    regex.push_str("\\s+");
                }
                // push ch to word
                word.push(ch);
                word_commited = false;
                prev_ch = ch;
            }
        }
    }

    regex
}

#[cfg(test)]
mod tests {
    use crate::{_get_regex_for_atom, QueryGroup, search_text};

    #[test]
    fn test_get_regex_for_atom() {
        assert_eq!(_get_regex_for_atom(""), "");
        assert_eq!(_get_regex_for_atom("  "), "");
        assert_eq!(_get_regex_for_atom("      "), "");
        assert_eq!(_get_regex_for_atom("hello"), "hello");
        assert_eq!(_get_regex_for_atom("hello world"), "hello\\s+world");
        assert_eq!(_get_regex_for_atom("hello world    again"), "hello\\s+world\\s+again");
        assert_eq!(_get_regex_for_atom("  hello world    "), "hello\\s+world");
        assert_eq!(_get_regex_for_atom("中"), "中");
        assert_eq!(_get_regex_for_atom("中文"), "中\\s*文");
        assert_eq!(_get_regex_for_atom("   中 文"), "中\\s+文");
        assert_eq!(_get_regex_for_atom("     中 文   "), "中\\s+文");
        assert_eq!(_get_regex_for_atom("中hello"), "中\\s*hello");
        assert_eq!(_get_regex_for_atom("中文hello"), "中\\s*文\\s*hello");
        assert_eq!(_get_regex_for_atom("中文hello world"), "中\\s*文\\s*hello\\s+world");
        assert_eq!(_get_regex_for_atom("中文 hello world"), "中\\s*文\\s*hello\\s+world");
        assert_eq!(_get_regex_for_atom("中文    hello  world"), "中\\s*文\\s*hello\\s+world");
        assert_eq!(_get_regex_for_atom("hello中"), "hello\\s*中");
        assert_eq!(_get_regex_for_atom("hello中文"), "hello\\s*中\\s*文");
        assert_eq!(_get_regex_for_atom("hello world中文"), "hello\\s+world\\s*中\\s*文");
        assert_eq!(_get_regex_for_atom("hello   world 中文"), "hello\\s+world\\s*中\\s*文");
        assert_eq!(_get_regex_for_atom("hello   world   中文"), "hello\\s+world\\s*中\\s*文");
        assert_eq!(_get_regex_for_atom("hello world中文again"), "hello\\s+world\\s*中\\s*文\\s*again");
        assert_eq!(_get_regex_for_atom("hello world  中文 again"), "hello\\s+world\\s*中\\s*文\\s*again");
        assert_eq!(_get_regex_for_atom("  hello world  中文 again  "), "hello\\s+world\\s*中\\s*文\\s*again");
        assert_eq!(_get_regex_for_atom("中文hello world世界"), "中\\s*文\\s*hello\\s+world\\s*世\\s*界");
        assert_eq!(_get_regex_for_atom("  中文 hello world世界   "), "中\\s*文\\s*hello\\s+world\\s*世\\s*界");
        assert_eq!(_get_regex_for_atom(" 中文hello world   世界"), "中\\s*文\\s*hello\\s+world\\s*世\\s*界");
    }

    #[test]
    fn test_search_text() {
        let query_group = QueryGroup::new(
            "world".to_string(),
            vec![]);
        let paths = vec![String::from("sample_texts/hello.txt"), String::from("sample_texts/world.txt")];
        let result = search_text(&query_group, &paths, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result.iter().next(), Some(&&String::from("sample_texts/world.txt")));

        let query_group = QueryGroup::new(
            "bar".to_string(),
            vec![vec!["baz".to_string(), "xxxx哈哈".to_string()]]);
        let paths = vec![String::from("sample_texts/hello.txt"), String::from("sample_texts/world.txt")];
        let result = search_text(&query_group, &paths, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result.iter().next(), Some(&&String::from("sample_texts/hello.txt")));
    }
}
