# journal-web-ui

Minimal web interface to query logs from `journalctl` or `docker logs`.


## Usage

The service is used by making HTTP requests to port 5000, e.g. to <http://localhost:5000/>. It will respond with the logs as plain text. The set of display logs can be customised by adding one or more of the following query parameters:

| parameter | description | example | provider |
| --------- | ----------- | ------- | ---- |
| `provider` | log provider (`journalctl` (default) or `docker`) | `provider=docker` | - |
| `lines` | number of lines to be shown or `all` to remove the limit | `lines=100` | all |
| `hostname` | limits the output to the given hostname | `hostname=the-name-of-the-host` |`journalctl`|
| `unit` | limits the output to the given systemd unit | `unit=systemd-journald.service` |`journalctl`|
| `since` | defines the start date of the analysis | `since=-24h` |all|
| `until` | defines the end date of the analysis | `until=2023-10-19 12:00` |all|
| `grep` | filters the messages for given regular expression | `grep=ERROR` |`journalctl`|

For example, to query all messages of `harvester.service` unit running on the `metadaten` machine logged in the last 24 hours and containing the pattern `ERROR`, use the following URL:

```
http://localhost:5000/?lines=all&hostname=metadaten&unit=harvester.service&grep=ERROR&since=-24h
```

You may change listening port/address with environment variable `BIND_ADDR` (e.g. `BIND_ADDR=[::]:5000`).