use crate::code_queries::{CodeQueries, QueryResults};
use std::collections::HashMap;
use std::io;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

#[derive(Debug)]
pub struct Aggregator<'a> {
    queries: &'a CodeQueries,
    results: HashMap<String, HashMap<String, usize>>,
}

impl<'a> Aggregator<'a> {
    pub fn new(queries: &'a CodeQueries) -> Self {
        Self {
            queries,
            results: HashMap::new(),
        }
    }

    pub fn add(&mut self, results: QueryResults) {
        let identifier = format!("{}/{}", results.repo_owner, results.repo_name);
        self.results.insert(identifier, results.inner);
    }

    pub async fn write(self, path: &Path) -> io::Result<()> {
        let f = File::create(path).await?;
        let mut writer = BufWriter::new(f);

        // header
        writer.write_all("repo".as_bytes()).await?;
        for query in self.queries.iter() {
            writer.write_u8(b',').await?;
            writer.write_all(query.as_bytes()).await?
        }

        writer.write_u8(b'\n').await?;

        // per repo results
        for (repo, results) in self.results.iter() {
            writer.write_all(repo.as_bytes()).await?;

            for query in self.queries.iter() {
                writer.write_u8(b',').await?;

                let count = results.get(query).unwrap_or(&0);
                writer.write_all(count.to_string().as_bytes()).await?;
            }

            writer.write_u8(b'\n').await?;
        }

        writer.flush().await?;

        Ok(())
    }
}
