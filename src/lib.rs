/*!
 * Implements the guts of bugcrawl.
 */

use reqwest::Client;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;
use time::Timespec;

const BUGCRAWL_USER_AGENT: &str = "bugcrawl";
const BUGVIEW_REQUEST_TIMEOUT: u64 = 30000;
const BUGVIEW_CONNECT_TIMEOUT: u64 = 30000;

pub struct BugcrawlParams<'a> {
    pub dbpath: &'a str,
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

struct Bugcrawl {
    /** path to the local sqlite database */
    dbpath: PathBuf,
    /**
     * if `readonly`, report bugs that have been updated, but do not update the
     * database.
     */
    readonly: bool,
    /** connection to the sqlite database */
    sqlite: Connection,
    /** HTTP client for the bugview API */
    bugview_client: Client,

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
    let mut dbpath = PathBuf::new();
    dbpath.push(params.dbpath);

    let sqlite_flags = match params.readonly {
        true => OpenFlags::SQLITE_OPEN_READ_ONLY,
        false => {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        }
    };

    let conn = Connection::open_with_flags(dbpath.clone(), sqlite_flags)?;

    let client = Client::builder()
        .timeout(Duration::from_millis(BUGVIEW_REQUEST_TIMEOUT))
        .connect_timeout(Duration::from_millis(BUGVIEW_CONNECT_TIMEOUT))
        .user_agent(BUGCRAWL_USER_AGENT)
        .build()?;

    let mut bcp = Bugcrawl {
        dbpath: dbpath,
        readonly: params.readonly,
        sqlite: conn,
        bugview_client: client,
        initial_latest_update: None,
        ntotalbugs: 0,
        ndbupdated: 0,
    };

    verify_db(&mut bcp);

    Ok(())
}

struct DbMetadataItem {
    name: String,
    value: String,
}

fn verify_db(bcp: &mut Bugcrawl) -> Result<(), BugcrawlError> {
    unimplemented!();
}
