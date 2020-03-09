/*!
 * Error handling facilities
 */

use std::fmt;

/**
 * BugcrawlError wraps any error we might encounter as part of the crawl.
 */
#[derive(Debug)]
pub struct BugcrawlError {
    pub message: String,
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

impl From<std::io::Error> for BugcrawlError {
    fn from(error: std::io::Error) -> BugcrawlError {
        BugcrawlError {
            message: format!("request error: {}", error),
        }
    }
}
