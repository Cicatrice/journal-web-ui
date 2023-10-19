# journal-web-ui

A service which provides a web UI for the systemd journal.

Web-based access to systems logs can be useful as it avoids the requirement of SSH access to the host collecting the logs which is beneficial for the security of the system, as there are fewer access points.

The implementation used [`journalctl (1)`](https://manpages.ubuntu.com/manpages/jammy/en/man1/journalctl.1.html) to access the system logs.

## Installation

1. The CI here builds a ready-to-use Debian package availabl at <https://gitlab.opencode.de/api/v4/projects/628/jobs/artifacts/main/download?job=build>.
2. This package needs to be installed on the target machine and will listen for HTTP connections on port 5000.
3. The set of expected log messages (which are filtered out if requested) needs to be configured in `/etc/expected-log-messages`.

## Usage

The service is used by making HTTP requests to port 5000, e.g. to <http://localhost:5000/>. It will respond with the logs as plain text. The set of display logs can be customised by adding one or more of the following query parameters:

| parameter | description | example |
| --------- | ----------- | ------- |
| lines | number of lines to be shown or `all` to remove the limit | `lines=100` |
| hostname | limits the output to the given hostname | `hostname=the-name-of-the-host` |
| unit | limits the output to the given systemd unit | `unit=systemd-journald.service` |
| since | defines the start date of the analysis | `since=-24h` |
| until | defines the end date of the analysis | `until=2023-10-19 12:00` |
| grep | filters the messages for given regular expression | `grep=ERROR` |
| unexpected | filters the messages using the configured set of expected log messages | `unexpected=` |

For example, to query all messages of `harvester.service` unit running on the `metadaten` machine logged in the last 24 hours and containing the pattern `ERROR`, use the following URL:

```
http://localhost:5000/?lines=all&hostname=metadaten&unit=harvester.service&grep=ERROR&since=-24h
```

The configuration file `/etc/expected-log-messages` defines which log messages are considered part of normal operations.

Using the `unexpected` parameter, the log messages can be limited to the unexpected ones, i.e. get [`logcheck (8)`](https://manpages.ubuntu.com/manpages/jammy/en/man8/logcheck.8.html)-like functionality.

The syntax of this file is line-oriented. Lines starting with a `#` are considered comments. Everything else is compiled as a regular expression following the syntax described at <https://docs.rs/regex/latest/regex/#syntax>.
