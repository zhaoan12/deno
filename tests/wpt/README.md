# Web Platform Tests (WPT)

Deno uses a custom test runner for Web Platform Tests. It can be found at
`./tests/wpt/wpt.ts`, relative to the root of this codebase.

## Setup

Before attempting to run WPT tests for the first time, run the setup command.
You must also run this command every time the `./tests/wpt/suite` submodule is
updated:

```shell
./tests/wpt/wpt.ts setup
```

This will:

- Check that Python 3.11 is available (required by the WPT test server)
- Update the WPT manifest (`./tests/wpt/runner/manifest.json`)
- Configure `/etc/hosts` with entries required by the WPT test server

You can specify the following flags:

- `--rebuild` — Rebuild the manifest from scratch instead of incrementally
  updating. This can take up to 3 minutes.
- `--auto-config` — Automatically configure `/etc/hosts` without prompting.

## Running tests

To run all web platform tests, use the `--all` flag:

```shell
./tests/wpt/wpt.ts run --all
```

To run a specific subset, specify filters after `--`:

```shell
# Run all tests in a suite
./tests/wpt/wpt.ts run -- fetch

# Run tests in a subdirectory
./tests/wpt/wpt.ts run -- streams/piping/general

# Run a single test file
./tests/wpt/wpt.ts run -- /WebCryptoAPI/getRandomValues.any.html

# Run multiple filters
./tests/wpt/wpt.ts run -- hr-time fetch/api/basic
```

Running `wpt.ts run` with neither `--all` nor filters will print usage help.

Filters can start with `/` (absolute path match) or without (prefix match
without the leading `/`).

Tests are run in parallel across CPU cores, partitioned by top-level directory.
If the WPT server is slow to boot in your environment, you can increase the
startup probe deadline with `DENO_WPT_SERVER_TIMEOUT_MS=<ms>`.

### Flags

- `--all` — Run all tests (required if no filters are specified)
- `--release` — Use `./target/release/deno` instead of `./target/debug/deno`
- `--binary=<path>` — Use a specific Deno binary (skips `cargo build`)
- `--quiet` — Only print failing test cases
- `--jobs=<n>` — Limit how many test partitions run in parallel
- `--retries=<n>` — Retry failing tests up to `n` attempts
- `--timeout-scale=<n>` — Multiply per-test timeouts by `n`
- `--file-timeout-ms=<n>` — Override the full-file timeout budget
- `--fail-fast` — Stop after the first unexpected failure
- `--json=<file>` — Write test results as JSON
- `--wptreport=<file>` — Write results in the
  [wptreport](https://github.com/nicedoc/wpt-report) format
- `--inspect-brk` — Attach the V8 inspector to each test
- `--verbose-server` — Stream the WPT server's stdout and stderr
- `--no-ignore` — Run tests marked with `{"ignore": true}` in expectations
- `--exit-zero` — Exit with code 0 even if there are failures

## Updating expectations

The `update` command runs tests and overwrites the expectation files to match
current results:

```shell
# Update all expectations
./tests/wpt/wpt.ts update --all

# Update expectations for specific suites
./tests/wpt/wpt.ts update -- hr-time fetch
```

Running `wpt.ts run` immediately after `wpt.ts update` should always pass.

The `update` command accepts the same flags as `run` (`--release`, `--binary`,
`--quiet`, `--json`, `--no-ignore`, `--inspect-brk`).

JSON output includes the file path, URL, manifest timeout class, normalized
expectation metadata, retry attempt count, harness result, stderr, and detailed
subtest statuses.

## Listing tests

Use the `list` command to inspect which files match a set of filters without
running them:

```shell
# List all tests in a suite
./tests/wpt/wpt.ts list -- fetch

# List a single test file
./tests/wpt/wpt.ts list -- /WebCryptoAPI/getRandomValues.any.html

# List the entire suite
./tests/wpt/wpt.ts list --all
```

Passing `--json=<file>` to `list` writes the discovered file paths, URLs,
expectations, and timeout classes without starting the WPT server.

Use `./tests/wpt/wpt.ts list-skipped` to inspect manifest tests that are being
filtered out by hard-coded runner skip rules such as HTTP/2-only coverage or
unsupported worker variants. It accepts the same trailing filters as `list`,
and `--json=<file>` writes the skipped paths with their exclusion reasons.

## Expectation file format

The expectations directory (`./tests/wpt/runner/expectations/`) contains one
JSON file per test suite (e.g., `fetch.json`, `dom.json`, `WebCryptoAPI.json`).
Each file is a nested JSON object that mirrors the WPT directory structure,
following the directory tree down to individual test files.

Leaf values describe what is expected for each test file:

| Value                                      | Meaning                                                               |
| ------------------------------------------ | --------------------------------------------------------------------- |
| `true`                                     | All subtests are expected to pass                                     |
| `false`                                    | The entire test file is expected to fail (crash, harness error, etc.) |
| `{"expectedFailures": ["name1", "name2"]}` | These specific subtests are expected to fail; all others should pass  |
| `{"ignore": true}`                         | Skip this test entirely (override with `--no-ignore`)                 |

Example:

```jsonc
{
  "fetch": {
    "api": {
      "basic": {
        "accept-header.any.html": true, // all subtests pass
        "stream-response.any.html": false, // entire file fails
        "request-headers.any.html": { // these 2 subtests fail
          "expectedFailures": [
            "Fetch with PUT with body",
            "Fetch with POST with body"
          ]
        },
        "mode-no-cors.sub.any.html": { // skipped
          "ignore": true
        }
      }
    }
  }
}
```

When the `run` command finishes, it shows a git diff between the current
expectation files and what the actual results would produce. This makes it easy
to see regressions and improvements.

## FAQ

### Upgrading the WPT submodule

```shell
cd tests/wpt/suite
git fetch origin
git checkout origin/epochs/daily
cd ../../../
git add ./tests/wpt/suite
```

All contributors will need to rerun `./tests/wpt/wpt.ts setup` after this.

Since upgrading WPT usually requires updating the expectations to cover upstream
changes, it's best to do that as a separate PR rather than as part of a PR that
implements a fix or feature.
