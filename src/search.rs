use crate::code_queries::{CodeQueries, QueryResults};
use anyhow::Result;
use grep::matcher::Matcher;
use grep::regex::{RegexMatcher, RegexMatcherBuilder};
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};
use std::collections::HashMap;
use std::io;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug)]
struct CounterSink<'a> {
    matcher: &'a RegexMatcher,
    matches: HashMap<String, usize>,
}

impl Sink for CounterSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch) -> Result<bool, Self::Error> {
        let mut matches = Vec::new();
        self.matcher.find_iter(mat.bytes(), |m| {
            matches.push(m);
            true
        })?;

        for m in matches {
            let s = std::str::from_utf8(&mat.bytes()[m.start()..m.end()]).unwrap();
            let count = self.matches.entry(s.to_owned()).or_insert(0);
            *count += 1;
        }

        Ok(true)
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn not_hidden(entry: &DirEntry) -> bool {
    !is_hidden(entry)
}

pub async fn search_repo(
    path: &Path,
    repo_owner: String,
    repo_name: String,
    queries: &CodeQueries,
) -> Result<QueryResults> {
    let matcher = RegexMatcherBuilder::new()
        .word(true)
        .build_literals(queries.as_slice())?;
    let mut searcher = SearcherBuilder::new()
        .line_number(false)
        .multi_line(false)
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();

    let mut sink = CounterSink {
        matcher: &matcher,
        matches: HashMap::new(),
    };

    let walker = WalkDir::new(path).into_iter();
    for result in walker.filter_entry(not_hidden) {
        let dir_entry = result?;
        if !dir_entry.file_type().is_file() {
            continue;
        }

        tokio::task::yield_now().await;
        searcher.search_path(&matcher, dir_entry.path(), &mut sink)?;
    }

    let results = QueryResults {
        repo_name,
        repo_owner,
        inner: sink.matches,
    };

    Ok(results)
}
