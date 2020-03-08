/*!
 * Implements the guts of bugcrawl.
 *
 * Key URLs examples:
 *
 * * list endpoint:
 *   `https://smartos.org/bugview/index.json?offset=0&sort=updated'
 * * individual issue: `https://smartos.org/bugview/fulljson/MANATEE-400`
 *
 * The `sort` value can be `updated`, `created`, or `key`.
 */

use reqwest::Client;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use time::Timespec;

const BUGCRAWL_USER_AGENT: &str = "bugcrawl";
const BUGVIEW_REQUEST_TIMEOUT: u64 = 30000;
const BUGVIEW_CONNECT_TIMEOUT: u64 = 30000;
const MAX_ISSUE_LEN: usize = 1024 * 1024;
const DBG_REQ: bool = false;
const DBG_ISSUE: bool = true;

pub struct BugcrawlParams<'a> {
    pub filepath: &'a str,
    pub readonly: bool,
}

#[derive(Debug)]
pub struct BugcrawlError {
    message: String,
}

impl fmt::Display for BugcrawlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<reqwest::Error> for BugcrawlError {
    fn from(error: reqwest::Error) -> BugcrawlError {
        BugcrawlError {
            message: format!("request error: {}", error),
        }
    }
}

impl From<rusqlite::Error> for BugcrawlError {
    fn from(error: rusqlite::Error) -> BugcrawlError {
        BugcrawlError {
            message: format!("request error: {}", error),
        }
    }
}

impl From<std::io::Error> for BugcrawlError {
    fn from(error: std::io::Error) -> BugcrawlError {
        BugcrawlError {
            message: format!("request error: {}", error),
        }
    }
}

pub struct Bugcrawl {
    /** path to the local directory of bug files */
    filepath: PathBuf,
    /**
     * if `readonly`, report bugs that have been updated, but do not update the
     * database.
     */
    readonly: bool,
    /** HTTP client for the bugview API */
    bugview_client: Client,
    /** tokio runtime */
    tokio_runtime: tokio::runtime::Runtime,

    /**
     * last update time of the most recently updated record in the database
     * when we started
     */
    initial_latest_update: Option<Timespec>,
    /** total number of bugs reported by bugview */
    ntotalbugs: usize,
    /** number of bugs updated on bugview relative to our database */
    ndbupdated: usize,
}

pub fn bugcrawl(params: BugcrawlParams) -> Result<(), BugcrawlError> {
    let mut filepath = PathBuf::new();
    filepath.push(params.filepath);

    let sqlite_flags = match params.readonly {
        true => OpenFlags::SQLITE_OPEN_READ_ONLY,
        false => {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        }
    };

    let client = Client::builder()
        .timeout(Duration::from_millis(BUGVIEW_REQUEST_TIMEOUT))
        .connect_timeout(Duration::from_millis(BUGVIEW_CONNECT_TIMEOUT))
        .user_agent(BUGCRAWL_USER_AGENT)
        .build()?;

    let runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let mut bcp = Bugcrawl {
        filepath: filepath,
        readonly: params.readonly,
        bugview_client: client,
        tokio_runtime: runtime,
        initial_latest_update: None,
        ntotalbugs: 0,
        ndbupdated: 0,
    };

    init_directory(&bcp)?;
    eprintln!("fetching full list of issue ids");
    let issue_ids = list_issues(&mut bcp)?;
    eprintln!("total issues: {}", issue_ids.len());
    eprintln!("determining which issues we already have");
    let new_issue_ids = issue_ids.iter().filter(|issue_id| {
        let newpath = path_for_issue(&bcp, issue_id, false);
        match std::fs::metadata(&newpath.as_path()) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Ok(_) => false,
            // XXX Is there a way to propagate this cleanly?
            Err(e) => panic!("failed to get local metadata for {}: {}",
                newpath.as_path().display(), e)
        }
    }).collect::<Vec<&String>>();
    eprintln!("total issues:       {}", issue_ids.len());
    eprintln!("issues to download: {}", new_issue_ids.len());

    let mut count = 0;
    for issue_id in new_issue_ids.iter() {
        count = count + 1;

        if DBG_ISSUE {
            eprintln!("downloading: {}", issue_id);
        }
        download_issue(&mut bcp, issue_id)?;
        if count % 100 == 1 {
            eprintln!("downloaded {} issues", count);
        }
    }

    Ok(())
}

pub fn init_directory(bcp: &Bugcrawl) -> Result<(), BugcrawlError> {
    Ok(fs::create_dir_all(bcp.filepath.as_path())?)
}

#[derive(Deserialize)]
struct IssueListPage {
    offset: usize,
    total: usize,
    sort: String,
    issues: Vec<IssueListItem>,
}

#[derive(Deserialize)]
struct IssueListItem {
    id: String,
    key: String,
    synopsis: String,
    resolution: Option<String>,
    updated: String,
    created: String,
}

fn list_issues(mut bcp: &mut Bugcrawl) -> Result<Vec<String>, BugcrawlError> {
    let baseurl =
        reqwest::Url::parse("https://smartos.org/bugview/index.json?").unwrap();
    let mut offset: usize = 0;
    let mut issue_ids: Vec<String> = Vec::new();

    loop {
        let page = list_issues_page(&mut bcp, &baseurl, "created", offset)?;
        process_issue_page(&bcp, &page, &mut issue_ids)?;
        offset = page.offset + page.issues.len();
        if page.offset + page.issues.len() >= page.total {
            break;
        }
    }

    Ok(issue_ids)
}

#[derive(Serialize)]
struct IssueListQueryParams<'a> {
    sort: &'a str, // TODO-cleanup: enum
    offset: usize,
}

fn list_issues_page(
    bcp: &mut Bugcrawl,
    baseurl: &reqwest::Url,
    sort_field: &str,
    offset: usize,
) -> Result<IssueListPage, BugcrawlError> {
    let url = baseurl.clone();
    let params = IssueListQueryParams {
        sort: sort_field,
        offset: offset,
    };
    let client = &bcp.bugview_client;
    let request = client.get(url).query(&params).build()?;
    let runtime = &mut bcp.tokio_runtime;
    if DBG_REQ {
        eprintln!("-> {} {}", request.method(), request.url());
    }
    let response = runtime.block_on(async {
        client.execute(request).await
    })?;
    let status = response.status();
    if DBG_REQ {
        eprintln!(
            "<- status {} {}",
            status.as_str(),
            status
                .canonical_reason()
                .unwrap_or("unknown response code")
        );
    }

    if !status.is_success() {
        return Err(BugcrawlError {
            message: format!("unexpected response code: {}", status),
        });
    }

    let page: IssueListPage = runtime.block_on(async {
        response.json().await
    })?;
    eprintln!("listed {} of {} total issues", page.offset, page.total);
    std::thread::sleep_ms(500);
    Ok(page)
}

fn process_issue_page(bcp: &Bugcrawl, page: &IssueListPage,
    issue_ids: &mut Vec<String>)
    -> Result<(), BugcrawlError>
{
    for item in page.issues.iter() {
        issue_ids.push(item.key.clone());
    }

    Ok(())
}

fn path_for_issue(bcp: &Bugcrawl, issue_id: &str, tmp: bool)
    -> std::path::PathBuf
{
    let mut newpath = std::path::PathBuf::new();
    newpath.push(&bcp.filepath);
    // XXX sanity-check for invalid characters
    newpath.push(format!("{}.json{}", issue_id, if tmp { ".tmp" } else { "" }));
    newpath
}

fn download_issue(bcp: &mut Bugcrawl, issue_id: &String)
    -> Result<(), BugcrawlError>
{
    let client = &bcp.bugview_client;
    let runtime = &mut bcp.tokio_runtime;
    // XXX check for invalid characters
    let url = reqwest::Url::parse(
        format!("https://smartos.org/bugview/fulljson/{}", issue_id).as_str()).unwrap();
    let request = client.get(url).build()?;
    // XXX commonize
    if DBG_REQ {
        eprintln!("-> {} {}", request.method(), request.url());
    }

    let response = runtime.block_on(async {
        client.execute(request).await
    })?;
    let status = response.status();
    if DBG_REQ {
        eprintln!(
            "<- status {} {}",
            status.as_str(),
            status
                .canonical_reason()
                .unwrap_or("unknown response code")
        );
    }

    if !status.is_success() {
        return Err(BugcrawlError {
            message: format!("unexpected response code: {}", status),
        });
    }

    /*
     * We could stream this, but we don't want to handle anything that's too
     * big. TODO-hardening stop accumulating after a given number of bytes
     * too.
     */
    let content = runtime.block_on(async {
        response.text().await
    })?;

    if content.len() > MAX_ISSUE_LEN {
        return Err(BugcrawlError {
            message: format!("issue {} was too big ({} bytes, max is {} bytes)",
                issue_id, content.len(), MAX_ISSUE_LEN)
        });
    }

    let newpath = path_for_issue(&bcp, issue_id, false);
    let newpath_tmp = path_for_issue(&bcp, issue_id, true);
    std::fs::write(newpath_tmp.as_path(), content)?;
    std::fs::rename(newpath_tmp, newpath);
    std::thread::sleep_ms(3000);

    Ok(())
}
