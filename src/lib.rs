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
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

mod error;
pub use error::BugcrawlError;

/** "user-agent" header value for crawl requests */
const BUGCRAWL_USER_AGENT: &str = "bugcrawl";
/** maximum time in milliseconds for any HTTP request to bugview */
const BUGVIEW_REQUEST_TIMEOUT: u64 = 30000;
/** maximum time in milliseconds for connect attempts to bugview */
const BUGVIEW_CONNECT_TIMEOUT: u64 = 30000;
/** minimum time in milliseconds between "list" requests to bugview */
const BUGVIEW_DELAY_LIST: u64 = 500;
/** minimum time in milliseconds between "get issue" requests to bugview */
const BUGVIEW_DELAY_GET_ISSUE: u64 = 1500;
/** maximum allowed size for any issue's JSON blob */
const MAX_ISSUE_LEN: usize = 10 * 1024 * 1024;

/** print a debug message for each request */
const DBG_REQ: bool = false;
/* print a debug message for each issue downloaded */
const DBG_ISSUE: bool = true;

/**
 * BugcrawlParams is used by consumers (i.e., `main()`) to describe what they
 * want to do.
 */
pub struct BugcrawlParams<'a> {
    /** local directory into which to store issue contents */
    pub filepath: &'a str,
}

/**
 * Stores the runtime state of the Bugcrawl operation.
 */
pub struct Bugcrawl {
    /** path to the local directory of bug files */
    filepath: PathBuf,
    /** HTTP client for the bugview API */
    bugview_client: Client,
    /** tokio runtime */
    tokio_runtime: tokio::runtime::Runtime,
}

/**
 * Crawl the "bugview" web service.  Currently, results are stored into flat
 * files in params.filepath.
 */
pub fn bugcrawl(params: BugcrawlParams) -> Result<(), BugcrawlError> {
    let mut filepath = PathBuf::new();
    filepath.push(params.filepath);

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
        bugview_client: client,
        tokio_runtime: runtime,
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

/**
 * Initialize the directory into which we will store downloaded issue files.
 */
pub fn init_directory(bcp: &Bugcrawl) -> Result<(), BugcrawlError> {
    Ok(fs::create_dir_all(bcp.filepath.as_path())?)
}

/**
 * Representation of the "ListIssues" JSON response, which contains a page of
 * issue summary objects.
 */
#[derive(Deserialize)]
#[allow(dead_code)]
struct IssueListPage {
    offset: usize,
    total: usize,
    sort: String,
    issues: Vec<IssueListItem>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct IssueListItem {
    id: String,
    key: String,
    synopsis: String,
    resolution: Option<String>,
    updated: String,
    created: String,
}

/**
 * List all of the issues in bugview, returning a list of the identifiers.
 */
fn list_issues(mut bcp: &mut Bugcrawl) -> Result<Vec<String>, BugcrawlError> {
    let baseurl =
        reqwest::Url::parse("https://smartos.org/bugview/index.json?").unwrap();
    let mut offset: usize = 0;
    let mut issue_ids: Vec<String> = Vec::new();

    loop {
        let page = list_issues_page(&mut bcp, &baseurl, "created", offset)?;
        for item in page.issues.iter() {
            issue_ids.push(item.key.clone());
        }
        offset = page.offset + page.issues.len();
        if page.offset + page.issues.len() >= page.total {
            break;
        }
    }

    Ok(issue_ids)
}

/**
 * List one page worth of issues from bugview.
 */
fn list_issues_page(
    bcp: &mut Bugcrawl,
    baseurl: &reqwest::Url,
    sort_field: &str,
    offset: usize,
) -> Result<IssueListPage, BugcrawlError> {
    #[derive(Serialize)]
    struct IssueListQueryParams<'a> {
        sort: &'a str, // TODO-cleanup: enum
        offset: usize,
    }

    let url = baseurl.clone();
    let params = IssueListQueryParams {
        sort: sort_field,
        offset: offset,
    };
    let client = &bcp.bugview_client;
    let request = client.get(url).query(&params).build()?;
    let response = make_request(bcp, request)?;
    let runtime = &mut bcp.tokio_runtime;
    let page: IssueListPage = runtime.block_on(async {
        response.json().await
    })?;
    eprintln!("listed {} of {} total issues", page.offset, page.total);
    std::thread::sleep(Duration::from_millis(BUGVIEW_DELAY_LIST));
    Ok(page)
}

/**
 * Given an issue identifier, return the local filesystem path where we will
 * store the issue.  If `tmp` is set, return a temporary file name to be used
 * for this issue's content.
 */
fn path_for_issue(bcp: &Bugcrawl, issue_id: &str, tmp: bool)
    -> std::path::PathBuf
{
    let mut newpath = std::path::PathBuf::new();
    newpath.push(&bcp.filepath);
    // XXX sanity-check for invalid characters
    newpath.push(format!("{}.json{}", issue_id, if tmp { ".tmp" } else { "" }));
    newpath
}

/**
 * Download the contents of the specified issue to the corresponding local file.
 */
fn download_issue(mut bcp: &mut Bugcrawl, issue_id: &String)
    -> Result<(), BugcrawlError>
{
    let client = &bcp.bugview_client;
    // XXX check for invalid characters
    let url = reqwest::Url::parse(
        format!("https://smartos.org/bugview/fulljson/{}", issue_id).as_str()).unwrap();
    let request = client.get(url).build()?;
    let response = make_request(&mut bcp, request)?;

    /*
     * We could stream this, but we don't want to handle anything that's too
     * big. TODO-hardening stop accumulating after a given number of bytes
     * too.
     */
    let runtime = &mut bcp.tokio_runtime;
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
    std::fs::rename(newpath_tmp, newpath)?;
    std::thread::sleep(Duration::from_millis(BUGVIEW_DELAY_GET_ISSUE));
    Ok(())
}

fn make_request(bcp: &mut Bugcrawl, request: reqwest::Request)
    -> Result<reqwest::Response, BugcrawlError>
{
    let client = &bcp.bugview_client;
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

    Ok(response)
}
