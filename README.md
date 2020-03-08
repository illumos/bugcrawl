This repo holds initial work for `bugcrawl`, a tool to crawl the [SmartOS bugview
database](https://smartos.org/bugview).

Done so far:
* initial skeleton: executable itself; library interface that initializes a
  sqlite database and HTTP client and exposes an error type

TODO:
- implement `verify_db()`, which should look for metadata stored in the sqlite
  database indicating what major version of this tool it was created with.
  - If the metadata table is not found or the row for the version is not found,
    proceed with initializing the database.
- implement database initialization (in this order -- see `verify_db()`)
  - define a schema for the main table of bugs (i.e., what data should be
    first-classed; probably at least the ticket id and update time, maybe also
    the project name, create time, creator, reporter, or assignee
  - create the bugs table
  - create the metadata table and insert metadata for create time, version, etc.
- implement initial state load from database
  - fetch the most recent update time and load it into the struct
- implement crawl:
  - implement fetching the complete list of bugs.  (NOTE: this will have to be
    different from an incremental crawl because there's no obvious way to do
    this in reverse order, which means we'll update the most recent first, which
    means we won't be able to tell next time that we didn't finish.)
    - fetch a single page
      - serde type(s) for the results
    - fetch the next page
      - update query params for subsequent requests
    - put it together to fetch all of the items that we need to update
    - fetch an individual bug's page
      - define serde type(s)
      - define database type(s)
      - fetch URL, parse it, update database
- put it all together
  - min time between requests
- build a tool to render information about a bug
- bonus (rough priority order):
  - track all versions of each issue so that we never clobber anything
  - implement incremental crawl
  - update database with metadata when we start each crawl.  as we go, update a
    counter about rows fetched, etc.  update at the end with end time.
  - implement incremental initial crawl? (can use the above state)
  - normalize the database better for richer searches:
    - people associated with tickets in various ways
    - tickets linked to other tickets
