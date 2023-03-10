use std::collections::HashMap;
use std::io;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Debug, Default, Clone)]
pub struct CodeQueries {
    inner: Vec<String>,
}

impl CodeQueries {
    pub async fn from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let f = File::open(path).await?;
        let reader = BufReader::new(f);
        let mut lines = reader.lines();

        let mut this = Self::default();
        while let Some(line) = lines.next_line().await? {
            this.push(line.trim().to_owned());
        }

        if this.inner.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Empty query file!",
            ));
        }

        Ok(this)
    }

    fn push(&mut self, s: String) {
        self.inner.push(s)
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.inner.iter()
    }

    pub fn as_slice(&self) -> &[String] {
        self.inner.as_slice()
    }
}

#[derive(Debug)]
pub struct QueryResults {
    pub repo_name: String,
    pub repo_owner: String,
    pub inner: HashMap<String, usize>,
}
