//! Where the assemblies of one run come from: positional paths, an assembly list or a sample
//! sheet. Every mapping is checked before any output is created.
//!
//! A sample name is report metadata only. The path an input resolves to is what is read and
//! what skani receives as the query label, exactly as a positional path would be, because
//! skani's tie ordering depends on that label.
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

/// One assembly to analyse, in the order it was supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Input {
    /// The file read and hashed, and the query label given to skani.
    pub path: PathBuf,
    /// A reporting name supplied with `--sample-name` or a sample sheet.
    pub sample_name: Option<String>,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    Positional,
    /// A line of an `--input-list` or `--sample-sheet` file.
    Listed {
        kind: &'static str,
        file: PathBuf,
        line: usize,
        entry: String,
    },
}

impl Input {
    /// The `source` object recorded under a report's `input`.
    pub(crate) fn source_value(&self) -> Value {
        match &self.source {
            Source::Positional => json!({"kind": "positional"}),
            Source::Listed {
                kind,
                file,
                line,
                entry,
            } => json!({"kind": kind, "file": file.to_string_lossy(), "line": line,
                "entry": entry}),
        }
    }
}

const INPUT_LIST: &str = "input_list";
const SAMPLE_SHEET: &str = "sample_sheet";

/// The inputs of one run. The command-line parser already refuses combining input sources
/// and `--sample-name` with a list or sheet.
pub(crate) fn collect(
    positional: &[PathBuf],
    input_list: Option<&Path>,
    sample_sheet: Option<&Path>,
    sample_name: Option<&str>,
) -> Result<Vec<Input>, String> {
    if let Some(file) = input_list {
        return parse_list(&read(file, "--input-list")?, file);
    }
    if let Some(file) = sample_sheet {
        return parse_sheet(&read(file, "--sample-sheet")?, file);
    }
    if let Some(name) = sample_name {
        if positional.len() != 1 {
            return Err(format!(
                "--sample-name names exactly one assembly, but {} were given; use --sample-sheet to name several",
                positional.len()
            ));
        }
        check_name(name).map_err(|e| format!("--sample-name {e}"))?;
    }
    Ok(positional
        .iter()
        .map(|path| Input {
            path: path.clone(),
            sample_name: sample_name.map(str::to_owned),
            source: Source::Positional,
        })
        .collect())
}

fn read(file: &Path, option: &str) -> Result<Vec<u8>, String> {
    fs::read(file).map_err(|e| format!("cannot read {option} {}: {e}", file.display()))
}

/// One literal assembly path per nonblank line.
fn parse_list(bytes: &[u8], file: &Path) -> Result<Vec<Input>, String> {
    let at = |line: usize| format!("input list {} line {line}", file.display());
    let mut inputs = Vec::new();
    for (line, text) in lines(bytes, file, "input list")? {
        if text.contains('\0') {
            return Err(format!("{}: the path contains a NUL byte", at(line)));
        }
        inputs.push(Input {
            path: beside(file, text),
            sample_name: None,
            source: Source::Listed {
                kind: INPUT_LIST,
                file: file.into(),
                line,
                entry: text.into(),
            },
        });
    }
    if inputs.is_empty() {
        return Err(format!(
            "input list {} names no assembly; it has no nonblank line",
            file.display()
        ));
    }
    reject_duplicate_paths(&inputs, "input list")?;
    Ok(inputs)
}

/// A header `sample<TAB>path`, then one name and one path per nonblank row.
fn parse_sheet(bytes: &[u8], file: &Path) -> Result<Vec<Input>, String> {
    let at = |line: usize| format!("sample sheet {} line {line}", file.display());
    let mut rows = lines(bytes, file, "sample sheet")?.into_iter();
    let Some((line, header)) = rows.next() else {
        return Err(format!(
            "sample sheet {} is empty; it needs the header sample<TAB>path and at least one row",
            file.display()
        ));
    };
    if header != "sample\tpath" {
        return Err(format!(
            "{}: the header must be exactly sample<TAB>path",
            at(line)
        ));
    }
    let mut inputs = Vec::new();
    let mut names: BTreeMap<&str, usize> = BTreeMap::new();
    for (line, text) in rows {
        let fields: Vec<&str> = text.split('\t').collect();
        let [name, path] = fields[..] else {
            return Err(format!(
                "{}: expected 2 tab-separated fields, sample and path, found {}",
                at(line),
                fields.len()
            ));
        };
        check_name(name).map_err(|e| format!("{}: sample {e}", at(line)))?;
        if path.trim().is_empty() {
            return Err(format!("{}: the path is empty", at(line)));
        }
        if path.contains('\0') {
            return Err(format!("{}: the path contains a NUL byte", at(line)));
        }
        if let Some(first) = names.insert(name, line) {
            return Err(format!(
                "{}: duplicate sample name {name:?}, first given on line {first}",
                at(line)
            ));
        }
        inputs.push(Input {
            path: beside(file, path),
            sample_name: Some(name.into()),
            source: Source::Listed {
                kind: SAMPLE_SHEET,
                file: file.into(),
                line,
                entry: path.into(),
            },
        });
    }
    if inputs.is_empty() {
        return Err(format!(
            "sample sheet {} has a header but no sample rows",
            file.display()
        ));
    }
    reject_duplicate_paths(&inputs, "sample sheet")?;
    Ok(inputs)
}

/// The nonblank lines of a UTF-8 file with their 1-based numbers. A leading byte-order mark is
/// dropped and a line may end in CRLF. Everything else on a line is kept as written.
fn lines<'a>(bytes: &'a [u8], file: &Path, what: &str) -> Result<Vec<(usize, &'a str)>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| {
        let line = 1 + bytes[..e.valid_up_to()]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        format!(
            "{what} {} line {line}: the file is not valid UTF-8",
            file.display()
        )
    })?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    Ok(text
        .split('\n')
        .enumerate()
        .map(|(index, line)| (index + 1, line.strip_suffix('\r').unwrap_or(line)))
        .filter(|(_, line)| !line.trim().is_empty())
        .collect())
}

/// A listed path, taken relative to the directory of the file that lists it.
fn beside(file: &Path, entry: &str) -> PathBuf {
    match file.parent() {
        Some(parent) => parent.join(entry),
        None => PathBuf::from(entry),
    }
}

/// A name must say something and must fit on one line of a report.
fn check_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("name is empty".into());
    }
    if name.chars().any(char::is_control) {
        return Err(format!("name {name:?} contains a control character"));
    }
    Ok(())
}

/// Two entries that name one file, through a different spelling, a symbolic link or a hard
/// link, would analyse the same assembly twice under two identities.
fn reject_duplicate_paths(inputs: &[Input], what: &str) -> Result<(), String> {
    let mut by_file: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    let mut by_path: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for (index, input) in inputs.iter().enumerate() {
        let file = fs::metadata(&input.path)
            .ok()
            .and_then(|m| by_file.insert((m.dev(), m.ino()), index));
        let path = lexical(&input.path).and_then(|p| by_path.insert(p, index));
        if let Some(first) = file.or(path) {
            let line = |i: &Input| match i.source {
                Source::Listed { line, .. } => line,
                Source::Positional => 0,
            };
            return Err(format!(
                "{what} line {}: {} is the same assembly as line {}; each assembly may be listed once",
                line(input),
                input.path.display(),
                line(&inputs[first])
            ));
        }
    }
    Ok(())
}

/// An absolute path with `.` and `..` applied, for files that do not exist.
fn lexical(path: &Path) -> Option<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in std::env::current_dir().ok()?.join(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => resolved.push(other),
        }
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(inputs: &[Input]) -> Vec<(String, Option<String>, usize)> {
        inputs
            .iter()
            .map(|i| {
                let Source::Listed { line, .. } = i.source else {
                    panic!("listed input")
                };
                (
                    i.path.to_string_lossy().into_owned(),
                    i.sample_name.clone(),
                    line,
                )
            })
            .collect()
    }

    #[test]
    fn a_list_keeps_order_spaces_and_crlf_and_resolves_beside_the_list() {
        let inputs = parse_list(
            "\u{feff}b.fa\r\n\r\n  \n/abs/a b.fa\r\nsub/ünï.fa.gz\n\n".as_bytes(),
            Path::new("runs/list.txt"),
        )
        .unwrap();
        assert_eq!(
            listed(&inputs),
            [
                ("runs/b.fa".into(), None, 1),
                ("/abs/a b.fa".into(), None, 4),
                ("runs/sub/ünï.fa.gz".into(), None, 5),
            ]
        );
        assert_eq!(
            inputs[0].source_value(),
            json!({"kind": "input_list", "file": "runs/list.txt", "line": 1, "entry": "b.fa"})
        );
    }

    #[test]
    fn a_list_in_the_working_directory_gives_the_same_label_as_a_positional_path() {
        let inputs = parse_list(b"a.fa\n", Path::new("list.txt")).unwrap();
        assert_eq!(inputs[0].path, PathBuf::from("a.fa"));
    }

    #[test]
    fn a_list_keeps_leading_and_trailing_spaces_and_has_no_comment_syntax() {
        let inputs = parse_list(b" a.fa \n# b.fa\n", Path::new("l")).unwrap();
        assert_eq!(inputs[0].path, PathBuf::from(" a.fa "));
        assert_eq!(inputs[1].path, PathBuf::from("# b.fa"));
    }

    #[test]
    fn empty_lists_and_invalid_utf8_are_refused_with_their_line() {
        for text in [&b""[..], b"\n\r\n  \n", b"\xef\xbb\xbf"] {
            let error = parse_list(text, Path::new("l.txt")).unwrap_err();
            assert!(error.contains("names no assembly"), "{error}");
        }
        let error = parse_list(b"a.fa\nb\xff.fa\n", Path::new("l.txt")).unwrap_err();
        assert!(
            error.contains("line 2: the file is not valid UTF-8"),
            "{error}"
        );
        let error = parse_list(b"a\0.fa\n", Path::new("l.txt")).unwrap_err();
        assert!(error.contains("NUL"), "{error}");
    }

    #[test]
    fn duplicate_spellings_of_one_path_are_refused() {
        let error = parse_list(b"a.fa\n./x/../a.fa\n", Path::new("l.txt")).unwrap_err();
        assert!(
            error.contains("input list line 2") && error.contains("as line 1"),
            "{error}"
        );
    }

    #[test]
    fn symbolic_and_hard_links_to_one_file_are_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.fa");
        fs::write(&file, ">a\nACGT\n").unwrap();
        std::os::unix::fs::symlink(&file, dir.path().join("link.fa")).unwrap();
        fs::hard_link(&file, dir.path().join("hard.fa")).unwrap();
        let list = dir.path().join("l.txt");
        for second in ["link.fa", "hard.fa"] {
            let error = parse_list(format!("a.fa\n{second}\n").as_bytes(), &list).unwrap_err();
            assert!(error.contains("same assembly as line 1"), "{error}");
        }
    }

    #[test]
    fn a_sheet_maps_names_to_paths_in_order() {
        let inputs = parse_sheet(
            "sample\tpath\r\nB 2\tb.fa\r\n\nä\t../x/a b.fa\n".as_bytes(),
            Path::new("d/s.tsv"),
        )
        .unwrap();
        assert_eq!(
            listed(&inputs),
            [
                ("d/b.fa".into(), Some("B 2".into()), 2),
                ("d/../x/a b.fa".into(), Some("ä".into()), 4),
            ]
        );
        assert_eq!(inputs[1].source_value()["kind"], "sample_sheet");
        assert_eq!(inputs[1].source_value()["entry"], "../x/a b.fa");
    }

    #[test]
    fn malformed_sheets_are_refused() {
        for (text, expected) in [
            ("", "is empty"),
            ("sample\tpath\n", "no sample rows"),
            ("path\tsample\na\tb\n", "line 1: the header"),
            ("sample\tpath\textra\na\tb\tc\n", "line 1: the header"),
            ("sample\tpath\na\n", "line 2: expected 2"),
            ("sample\tpath\na\tb\tc\n", "line 2: expected 2"),
            ("sample\tpath\n\tb.fa\n", "line 2: sample name is empty"),
            ("sample\tpath\n \tb.fa\n", "line 2: sample name is empty"),
            ("sample\tpath\na\t\n", "line 2: the path is empty"),
            ("sample\tpath\na\x07\tb.fa\n", "control character"),
            (
                "sample\tpath\na\tb.fa\n\na\tc.fa\n",
                "line 4: duplicate sample name \"a\", first given on line 2",
            ),
            (
                "sample\tpath\na\tb.fa\nc\t./b.fa\n",
                "same assembly as line 2",
            ),
        ] {
            let error = parse_sheet(text.as_bytes(), Path::new("s.tsv")).unwrap_err();
            assert!(error.contains(expected), "{text:?}: {error}");
        }
    }

    #[test]
    fn a_sample_name_needs_exactly_one_positional_assembly() {
        let one = collect(&["a.fa".into()], None, None, Some("S1")).unwrap();
        assert_eq!(one[0].sample_name.as_deref(), Some("S1"));
        assert_eq!(one[0].source, Source::Positional);
        let error = collect(&["a.fa".into(), "b.fa".into()], None, None, Some("S1")).unwrap_err();
        assert!(error.contains("exactly one assembly, but 2"), "{error}");
        assert!(collect(&["a.fa".into()], None, None, Some(" ")).is_err());
    }

    #[test]
    fn positional_paths_keep_duplicates_and_have_no_name() {
        let inputs = collect(&["a.fa".into(), "a.fa".into()], None, None, None).unwrap();
        assert_eq!(inputs.len(), 2);
        assert!(inputs.iter().all(|i| i.sample_name.is_none()));
        assert_eq!(inputs[0].source_value(), json!({"kind": "positional"}));
    }
}
