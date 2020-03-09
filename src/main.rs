/*!
 * bugcrawl: crawl the bugview database
 */

/** Executable name */
const ARG0_DEFAULT: &str = "bugcrawl";
/** Canonical Unix exit status for a non-usage-related failure. **/
const EXIT_FAILURE: i32 = 1;

use bugcrawl::bugcrawl;
use bugcrawl::BugcrawlParams;

fn main()
{
    let params = BugcrawlParams {
        filepath: "./bugdb.files",
    };

    if let Err(error) = bugcrawl(params) {
        eprintln!("{}: {}", ARG0_DEFAULT, error);
        std::process::exit(EXIT_FAILURE);
    }
}
