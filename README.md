# bugcrawl

`bugcrawl` is a tool to crawl [bugview](https://smartos.org/bugview), which is a
publicly accessible view of issues related to Joyent's Manta, Triton, and
SmartOS projects.

`bugcrawl` uses the public URLs to list issues, pages through them all, and
downloads each one to a local file called `bugdb.files/$TICKET.json` where
`$TICKET` is the JIRA identifier (e.g., `OS-2312`).  If any such file exists
locally already, it skips that issue.  It doesn't do anything fancier or smarter
than that.  See "Status" below for more.


## Synopsis

You can build and run this tool like any other rust crate with `cargo build`
and/or `cargo run`.  Currently, the tool takes no arguments and immediately
starts crawling the public `bugview` database.  Be careful with this!  It's
single-threaded and pauses between requests to avoid disrupting the server, but
that doesn't mean it couldn't still cause trouble.

As of 2020-03, there are 6735 issues and they take up about 70MB of space
locally.  It takes a few hours to crawl the whole database, mostly because of
the delays between requests to avoid overwhelming the server.


## Status

Currently, `bugcrawl` pages through all issues and downloads each one to a local
file if it's not already present.  That's enough to produce a local copy of the
currently-public bug database.

It would be useful to do a few more things:

- Render the issues, maybe the same way `bugview` does.  The [Bugview
  source](https://github.com/joyent/bugview/) is publicly available and it
  should be possible to rework the files produced by `bugcrawl` to be rendered
  directly by `bugview` (or something similar).
- Support correct incremental updates from the public database.  The public
  database supports querying by last update time, so it would be possible to
  add a mode to `bugcrawl` to efficiently list changes it didn't already know
  about and update the local state.  (Today, if you rerun the program, it will
  fetch newly-created issues, but it won't fetch updates to issues that it
  already knew about.)
- Keep track of various versions of issues.  That is, if an update on bugview
  deleted an issue or modified the synopsis or the like, it would be nice to
  capture all the previous revisions instead of clobbering the local copy with
  what's upstream.  This is important for operating as a proper archive.

Other things that would be useful:

- polish around the CLI: arguments for the local path, dry run, delay between
  requests, etc.
- supporting a sqlite database (or the like) so that we could query issues by
  more fields even than bugview supports (e.g., author)
