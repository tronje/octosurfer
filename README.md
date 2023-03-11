# octosurfer

Searches GitHub for repositories, then clones each repository and greps through
it for a list of terms.

## Installation

- Make sure you have a working Rust toolchain installed.
- `cargo install --git https://github.com/tronje/octosurfer`

This project may be published to crates.io in the future, but hasn't yet.

## Usage

Make sure you generate a GitHub API key and export it as `GITHUB_TOKEN` in your
environment.

```
Usage: octosurfer -k <keywords> [-l <languages>] [-p <pushed>] [-s <stars>] [-t <topics>] -d <target-dir> -q <query-file> -o <out-file> [--rm] [-v <verbosity>]

Clone all GitHub repositories matching a query and search them

Options:
  -k, --keywords    keywords to use when searching for repos (comma-separated)
  -l, --languages   limit search to repos that use these languages
                    (comma-separated)
  -p, --pushed      limit search by date, e.g. ">1970-01-01" for repos updated
                    after Jan 1st, 1970
  -s, --stars       limit search by stars, e.g. ">100" for repos with more than
                    100 stars
  -t, --topics      limit search by these topics (comma-separated)
  -d, --target-dir  path to a directory into which repositories should be cloned
  -q, --query-file  file to read code queries from
  -o, --out-file    filename to write CSV results into
  --rm              remove repos after analysis is complete
  -v, --verbosity   sets the verbosity (off, error, warn, info, debug, or trace)
  --help            display usage information
```

## Example

The following invocation will search GitHub for all repositories that:
- include the keyword "mpi"
- are written primarily in either C or C++
- have been pushed to some time after Jan 1st, 2013
- have more than two stars

It will then clone each repository in `/tmp/octosurfer`, search each repository
for occurences of the queries in the file `my-queries.txt`, and save the results
in `results.csv`. Each cloned repository will be removed after it has been
searched.

```console
$ octosurfer -k mpi \
	-l c,c++ \
	-p ">2013-01-01" \
	-s ">2" \
	-d /tmp/octosurfer \
	-q my-queries.txt \
	-o results.csv \
	--rm
```

## Queries

Queries are listed in a text file, and the file name is given to `octosurfer`
with the `-q` flag. There should be one query per line, and regex syntax may
be used in a query. `octosurfer` searches files line by line, so there can be
no multiline matches.

## Performance

`octosurfer` uses [tokio](https://tokio.rs) and makes heavy use of `async` Rust.
Repositories are cloned and searched asynchronously. Searching through a single
repository is single-threaded, though. The assumption is that usually, multiple
repositories are searched at a time, so each search does not need to be parallelized.
During testing, heavy parallel searching raised the OS error `EMFILE`, i.e.
"too many open files".

`octosurfer` uses the [grep crate](https://crates.io/crates/grep) to search files.
This crate is the library that powers ripgrep.

## Disk usage

`octosurfer` clones each repository shallowly, i.e. with `git clone --depth 1`.
However, because the GitHub search can return hundreds or thousands of repositories,
the cumulative disk use can become quite significant. It may be prudent to pass
the `--rm` flag if unsure of how many repositories a search will yield.

Note that because repositories are cloned asynchronously, more than one repository
may exist on-disk at a time, even with the `--rm` flag given.

`octosurfer` clones repositories into the given target directory, and then under
`/{repo owner's name}/{repo name}`. If `--rm` is given, `octosurfer` will
remove the cloned repository, as well as the directory named after the repository
owner. It is therefore advisable to **pass an empty directory as the --target-dir flag**,
to avoid `octosurfer` accidentally removing files and directories you intended
to keep.
