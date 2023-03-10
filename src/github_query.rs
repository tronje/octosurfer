use crate::OctoSurfer;
use anyhow::{anyhow, Result};

fn split_opt_str(s: &Option<String>) -> Option<Vec<String>> {
    s.as_ref()
        .map(|s| s.split(',').map(ToOwned::to_owned).collect())
}

#[derive(Debug)]
pub struct GithubQuery {
    keywords: Vec<String>,
    languages: Option<Vec<String>>,
    pushed: Option<Vec<String>>,
    stars: Option<Vec<String>>,
    topics: Option<Vec<String>>,
}

impl GithubQuery {
    pub fn from_argh(argh: &OctoSurfer) -> Self {
        let keywords = argh.keywords.split(',').map(ToOwned::to_owned).collect();
        let languages = split_opt_str(&argh.languages);
        let pushed = split_opt_str(&argh.pushed);
        let stars = split_opt_str(&argh.stars);
        let topics = split_opt_str(&argh.topics);

        Self {
            keywords,
            languages,
            pushed,
            stars,
            topics,
        }
    }

    pub fn to_query_string(&self) -> Result<String> {
        let mut s = self.keywords.join(" ");

        if let Some(langs) = &self.languages {
            for lang in langs {
                s.push_str(" language:");
                s.push_str(lang);
            }
        }

        if let Some(pushed) = &self.pushed {
            for spec in pushed {
                s.push_str(" pushed:");
                s.push_str(spec);
            }
        }

        if let Some(stars) = &self.stars {
            for spec in stars {
                s.push_str(" stars:");
                s.push_str(spec);
            }
        }

        if let Some(topics) = &self.topics {
            for topic in topics {
                s.push_str(" topic:");
                s.push_str(topic);
            }
        }

        if s.chars().count() > 256 {
            Err(anyhow!("Query string exceeded 256 characters!"))
        } else {
            Ok(s)
        }
    }
}
