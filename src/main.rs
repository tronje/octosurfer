mod aggregate;
mod code_queries;
mod github_query;
mod search;

use crate::code_queries::{CodeQueries, QueryResults};
use crate::github_query::GithubQuery;
use anyhow::{anyhow, Context, Result};
use argh::FromArgs;
use chrono::TimeZone;
use octocrab::models::Repository;
use octocrab::Octocrab;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::task::JoinHandle;
use url::Url;

/// Clone all GitHub repositories matching a query and search them
#[derive(FromArgs)]
pub struct OctoSurfer {
    /// keywords to use when searching for repos (comma-separated)
    #[argh(option, short = 'k')]
    keywords: String,

    /// limit search to repos that use these languages (comma-separated)
    #[argh(option, short = 'l')]
    languages: Option<String>,

    /// limit search by date, e.g. ">1970-01-01" for repos updated after Jan 1st, 1970
    #[argh(option, short = 'p')]
    pushed: Option<String>,

    /// limit search by stars, e.g. ">100" for repos with more than 100 stars
    #[argh(option, short = 's')]
    stars: Option<String>,

    /// limit search by these topics (comma-separated)
    #[argh(option, short = 't')]
    topics: Option<String>,

    /// path to a directory into which repositories should be cloned
    #[argh(option, short = 'd')]
    target_dir: PathBuf,

    /// file to read code queries from
    #[argh(option, short = 'q')]
    query_file: PathBuf,

    /// filename to write CSV results into
    #[argh(option, short = 'o')]
    out_file: PathBuf,

    /// remove repos after analysis is complete
    #[argh(switch)]
    rm: bool,

    /// sets the verbosity (off, error, warn, info, debug, or trace)
    #[argh(option, short = 'v', default = "log::LevelFilter::Info")]
    verbosity: log::LevelFilter,
}

async fn update_repo(path: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path.as_os_str())
        .arg("pull")
        .output()
        .await?;

    if output.status.success() {
        log::debug!("Successfully updated {:?}", path);
        Ok(())
    } else {
        Err(anyhow!("Failed to update {:?}", path))
    }
}

async fn clone_repo(clone_path: &Path, owner: &str, name: &str, clone_url: &Url) -> Result<()> {
    tokio::fs::create_dir_all(&clone_path).await?;

    let output = Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg("--depth")
        .arg("1")
        .arg(clone_url.as_str())
        .arg(clone_path.as_os_str())
        .output()
        .await?;

    if output.status.success() {
        log::debug!("Successfully cloned {}/{}", owner, name);
        Ok(())
    } else {
        Err(anyhow!(
            "Failed to clone repo {}/{}! Exit code: {}",
            owner,
            name,
            output.status
        ))
    }
}

async fn handle_repo(
    repo: Repository,
    base: PathBuf,
    queries: CodeQueries,
    remove: bool,
) -> Result<QueryResults> {
    let name = &repo.name;
    let owner = &repo
        .owner
        .as_ref()
        .ok_or_else(|| anyhow!("Repo without an owner!"))?
        .login;
    let clone_url = &repo
        .clone_url
        .as_ref()
        .ok_or_else(|| anyhow!("Repo without a clone URL!"))?;

    let clone_path = base.join(owner).join(name);

    if tokio::fs::try_exists(&clone_path).await? {
        log::info!("Updating {}/{}", owner, name);
        update_repo(&clone_path).await?;
    } else {
        log::info!("Cloning {}/{}", owner, name);
        clone_repo(&clone_path, owner, name, clone_url).await?;
    }

    // try to avoid EMFILE (too many open files)
    tokio::time::sleep(Duration::from_millis(100)).await;

    let results =
        search::search_repo(&clone_path, owner.to_owned(), name.to_owned(), &queries).await?;

    if remove {
        log::debug!("Removing {:?}", clone_path);
        tokio::fs::remove_dir_all(&clone_path).await?;
    }

    Ok(results)
}

struct Runner {
    cli_app: OctoSurfer,
    octocrab: Octocrab,
    code_queries: CodeQueries,
    rm_paths: HashSet<PathBuf>,
}

impl Runner {
    async fn wait_for_reset(&self, reset_ts: u64) -> Result<()> {
        let reset_ts = reset_ts.try_into()?;
        let reset = chrono::Utc.timestamp_opt(reset_ts, 0).unwrap();
        let now = chrono::Utc::now();

        if reset > now {
            let delta = reset - now;
            let delta = delta.to_std()?;
            log::info!("Sleeping for {:?}", delta);
            tokio::time::sleep(delta).await;
        } else {
            log::warn!("Rate limit reset time was in the past!");
        }

        Ok(())
    }

    async fn check_rate_limit(&self) -> Result<()> {
        // GitHub gives 30 search requests per minute
        // https://docs.github.com/en/rest/search?apiVersion=2022-11-28

        let rate = self.octocrab.ratelimit().get().await?.resources.search;
        let remaining = rate.remaining;
        log::trace!("Remaining requests: {remaining}/30");

        if remaining == 0 {
            log::warn!("Search rate limit exhausted!");
            self.wait_for_reset(rate.reset).await?;
        } else if remaining < 10 {
            log::warn!("Running low on search requests: {remaining}/30");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        Ok(())
    }

    async fn handle_page(
        &mut self,
        repos: Vec<Repository>,
    ) -> Result<Vec<JoinHandle<Result<QueryResults>>>> {
        let mut handles = Vec::with_capacity(repos.len());

        for repo in repos {
            if self.cli_app.rm {
                let owner = &repo
                    .owner
                    .as_ref()
                    .ok_or_else(|| anyhow!("Repo without an owner!"))?
                    .login;
                let rm_path = self.cli_app.target_dir.join(owner);
                self.rm_paths.insert(rm_path);
            }

            let handle = tokio::spawn(handle_repo(
                repo,
                self.cli_app.target_dir.clone(),
                self.code_queries.clone(),
                self.cli_app.rm,
            ));
            handles.push(handle);
        }

        Ok(handles)
    }

    async fn run(&mut self) -> Result<()> {
        self.check_rate_limit().await?;

        let query_string = GithubQuery::from_argh(&self.cli_app).to_query_string()?;
        let mut page = self
            .octocrab
            .search()
            .repositories(&query_string)
            .sort("updated")
            .order("desc")
            .send()
            .await?;

        let mut handles = Vec::new();
        loop {
            let handle = self.handle_page(page.items).await?;
            handles.extend(handle);

            self.check_rate_limit().await?;

            match self.octocrab.get_page(&page.next).await? {
                Some(next_page) => {
                    page = next_page;
                }
                None => break,
            };
        }

        let mut aggregator = aggregate::Aggregator::new(&self.code_queries);

        let mut succeeded = 0;
        let mut failed = 0;

        for handle in handles {
            match handle.await? {
                Ok(results) => {
                    succeeded += 1;
                    aggregator.add(results);
                }

                Err(e) => {
                    log::error!("Failed: {e}");
                    failed += 1;
                }
            }
        }

        let total = succeeded + failed;
        log::info!("Checked {total} repos, of which {succeeded} succeeded and {failed} failed.");

        aggregator.write(&self.cli_app.out_file).await?;
        log::info!("Wrote results to {:?}", self.cli_app.out_file);

        // Repos are cloned to {target_dir}/{owner}/{repo}, and when they are removed after
        // searching, {target_dir}/{owner} remains! So clean that up here.
        if self.cli_app.rm {
            for path in self.rm_paths.iter() {
                log::info!("Removing {}", path.display());
                tokio::fs::remove_dir(path).await?;
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_app: OctoSurfer = argh::from_env();

    match cli_app.verbosity {
        log::LevelFilter::Off => {}
        _ => simple_logger::init_with_level(cli_app.verbosity.to_level().unwrap())?,
    }

    let gh_token =
        std::env::var("GITHUB_TOKEN").context("Must set GITHUB_TOKEN environment variable!")?;
    let octocrab = Octocrab::builder().personal_token(gh_token).build()?;

    let code_queries = CodeQueries::from_file(&cli_app.query_file).await?;

    let mut runner = Runner {
        cli_app,
        octocrab,
        code_queries,
        rm_paths: HashSet::new(),
    };

    runner.run().await
}
