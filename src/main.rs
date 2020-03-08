/*!
 * bugcrawl: crawl the bugview database
 *
 * Basic design:
 * - store local copies of bugs into sqlite?
 * - fetch most recently updated bug and date/time in sqlite
 * - fetch https://smartos.org/bugview/index.json?offset=0&sort=updated
 *   to see recently updated tickets
 * - continue walking back on index.json until we get to one we've seen before
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
        dbpath: "./bugdb.sqlite",
        readonly: false,
    };

    if let Err(error) = bugcrawl(params) {
        eprintln!("{}: {}", ARG0_DEFAULT, error);
        std::process::exit(EXIT_FAILURE);
    }
}
