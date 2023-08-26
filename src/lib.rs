use std::fs;
use regex::{Regex, RegexBuilder};
use rayon::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;


/// AND of patterns, where each pattern except for the first is the OR of some sub-patterns.
#[pyclass]
pub struct QueryGroup {
    pub patterns: Vec<Regex>,
}

#[pymethods]
impl QueryGroup {
    #[new]
    pub fn new(and_of_or_atoms: Vec<Vec<String>>) -> PyResult<Self> {
        let mut patterns = Vec::new();
        if and_of_or_atoms.is_empty() {
            return Err(PyValueError::new_err("query group must not be empty"));
        }
        for or_grp in and_of_or_atoms.iter() {
            patterns.push(get_regex_for_atoms(or_grp));
        }

        Ok(QueryGroup {
            patterns,
        })
    }
}

#[pyclass]
pub struct FileMatchResult {
    #[pyo3(get)]
    path: String,

    #[pyo3(get)]
    context: Option<String>,
}

fn is_match_str(query_group: &QueryGroup, contents: &str) -> bool {
    for pat in query_group.patterns.iter() {
        if !pat.is_match(contents) {
            return false;
        }
    }
    true
}

fn is_match(query_group: &QueryGroup, path: &str) -> Option<FileMatchResult> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            for pat in query_group.patterns.iter() {
                if !pat.is_match(&contents) {
                    return None;
                }
            }
            Some(FileMatchResult {
                path: String::from(path),
                context: None,
            })
        }
        Err(_) => None
    }
}

fn is_match_context(query_group: &QueryGroup, path: &str, a: usize, b: usize) -> Option<FileMatchResult> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let mut context = None;
            for (i, pat) in query_group.patterns.iter().enumerate() {
                if i == 0 {
                    match pat.find(&contents) {
                        None => return None,
                        Some(m) => {
                            let approx_start = if m.start() < a { 0 } else { m.start() - a };
                            let approx_end = if m.end() + b > contents.len() { contents.len() } else { m.end() + b };
                            context = Some(String::from(approx_substring(&contents, approx_start, approx_end)));
                        }
                    }
                } else {
                    if !pat.is_match(&contents) {
                        return None;
                    }
                }
            }
            Some(FileMatchResult {
                path: String::from(path),
                context,
            })
        }
        Err(_) => None
    }
}

pub fn search_text(query_group: &QueryGroup, textfile_paths: &[String], parallel: bool) -> Vec<FileMatchResult> {
    if parallel {
        textfile_paths
            .par_iter()
            .filter_map(|path| is_match(&query_group, path))
            .collect()
    } else {
        textfile_paths
            .iter()
            .filter_map(|path| is_match(&query_group, path))
            .collect()
    }
}

pub fn search_text_context(query_group: &QueryGroup, textfile_paths: &[String], a: usize, b: usize, parallel: bool) -> Vec<FileMatchResult> {
    if parallel {
        textfile_paths
            .par_iter()
            .filter_map(|path| is_match_context(&query_group, path, a, b))
            .collect()
    } else {
        textfile_paths
            .iter()
            .filter_map(|path| is_match_context(&query_group, path, a, b))
            .collect()
    }
}

fn approx_substring(
    contents: &str,
    approx_start_byte_index: usize,
    approx_end_byte_index: usize,
) -> &str {
    let end = contents.len();
    let mut start_byte_index = approx_start_byte_index;
    let mut end_byte_index = approx_end_byte_index;
    while start_byte_index <= end {
        if contents.is_char_boundary(start_byte_index) {
            break;
        }
        start_byte_index += 1;
    }
    // when `end_byte_index` reaches 0, it must break out of the loop
    loop {
        if contents.is_char_boundary(end_byte_index) {
            break;
        }
        end_byte_index -= 1;
    }
    if start_byte_index <= end_byte_index {
        &contents[start_byte_index..end_byte_index]
    } else {
        ""
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
pub fn py_search_text(query_group: &QueryGroup, textfile_paths: &FilePaths, a: Option<usize>, b: Option<usize>) -> Vec<FileMatchResult> {
    match (a, b) {
        (None, None) | (None, Some(_)) | (Some(_), None) => search_text(query_group, &textfile_paths.paths, true),
        (Some(a), Some(b)) => search_text_context(query_group, &textfile_paths.paths, a, b, true),
    }
}

#[pyfunction]
#[pyo3(name = "match_str")]
pub fn py_match_str(query_group: &QueryGroup, contents: &str) -> bool {
    is_match_str(query_group, contents)
}

#[pymodule]
#[pyo3(name = "textsearcher")]
fn py_module(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<QueryGroup>()?;
    m.add_class::<FilePaths>()?;
    m.add_function(wrap_pyfunction!(py_search_text, m)?)?;
    m.add_function(wrap_pyfunction!(py_match_str, m)?)?;
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

    // without Python package 'maturin', this test goes wrong false positively
    #[test]
    fn test_search_text() {
        let query_group = QueryGroup::new(
            vec![vec!["world".to_string()]]).unwrap();
        let paths = vec![String::from("sample_texts/hello.txt"), String::from("sample_texts/world.txt")];
        let result = search_text(&query_group, &paths, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result.iter().next().unwrap().path, String::from("sample_texts/world.txt"));

        let query_group = QueryGroup::new(
            vec![vec!["bar".to_string()],
                 vec!["baz".to_string(), "xxxx哈哈".to_string()]]).unwrap();
        let paths = vec![String::from("sample_texts/hello.txt"), String::from("sample_texts/world.txt")];
        let result = search_text(&query_group, &paths, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result.iter().next().unwrap().path, String::from("sample_texts/hello.txt"));
    }
}
