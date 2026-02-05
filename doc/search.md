# Search

Design document for full-text search through

- bookmark titles
- content of bookmarked html websites
- list titles
- list descriptions (called "content" in the code)

## Requirements

- No extra service to deploy
- Up to a certain size, search should take <500ms and be a lot faster for small datasets
- Index size should be reasonable, e.g. not more than 2x of original content
- target: 50 users per instance with 50k bookmarks each should be easy for hosters
- need to return matched positions for highlighting them in search results
- should support language-aware stemming
- Should have a "good" way to rank results

## "Good" Ranking

- Should some level of fuzziness be involved?
- Weight matches in the bookmark title higher than website content
- BM25 is a good baseline

## PostgreSQL full-text-search

- Seems like the ranking functions `ts_rank` and `ts_rank_cd` have pretty heavy performance impact, although for the size of linkblocks this might not be a problem
- No BM25 ranking, but it can at least normalize word frequency by document length
- tsvectors can only take a limited number of lexeme positions (but an unlimited number of lexemes?)
- Can rank different parts of the tsvector differently (title, body)
- ranking output is not normalized, so it might be tricky to compare list and bookmark ranks for the same query (so we can display both in the same search result list)
- quicker to implement than Tantivy
- Let's evaluate how good the ranking actually is

## Tantivy

- Uses BM25 for ranking
- Ranking is faster than with Postgres
- Extra implementation complexity if indexes are cached on disk, or extra memory & compute required if they are stored in-memory
    - Check the indexing performance & space requirements
- For on-disk storage, needs extra work to robustly handle file corruption, recovery from bugs, etc.
    - Alternatively: store indexes in postgres

## pg_search

- Implements BM25 and more in postgres using tantivy
- [Requires custom postgres installation](https://github.com/paradedb/paradedb/tree/main/pg_search#installation)
