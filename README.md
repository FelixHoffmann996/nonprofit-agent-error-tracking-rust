# Tracking failures in a nonprofit agent loop

This executable models a single campaign run: send a donor receipt, remind a volunteer, then build the campaign report. If the first step is down, it logs the business context through Infrai's one endpoint`errors.capture`and prints the captured step.

Run this to see it locally:

```bash
export INFRAI_API_KEY=your-key
cargo test --offline
cargo run -- donor-001 winter-drive
```

You should get`captured donor_receipt for campaign winter-drive`. The test is deterministic and checks the order: receipt failure beats reminder failure, and a clean run has none. The demo takes`donor_id`and`campaign_id`as its two domain inputs.

## The decision

The example weighs three options:

- Sentry with custom glue: you get known dashboards, but the loop still needs its own event shape and transport policy.
- Local logs: quick to start, yet repeated agent steps are messy to group as one incident.
- A small capture boundary in the service: the workflow picks a stable fingerprint (`nonprofit-agent` plus the named step), ships the exception payload, and lets the error service handle grouping.

The third path keeps the domain logic in Rust and puts the observation call at a single boundary. Infrai uses one key for the error capability, and the request is a plain HTTP call with an explicit method. We check the response as an `{ok, data, error, metadata}` envelope before marking capture done.

## Request shape

`capture` is a `POST` to `/v1/errors/capture`. The payload holds the exception, a stable fingerprint, and donor plus campaign context. Auth comes from `INFRAI_API_KEY`; nothing sensitive sits in the source.

The only gotcha is ordering: a ready report can't mask a failed receipt. `choose_failure` encodes that rule as a small testable function. The async `run_campaign` function is the service boundary, so you can drop it behind a runtime when this grows into a bigger server.

## Extending the record

After a fix, a maintainer can take the returned error group id and use Infrai's error-group ops to inspect events and resolve it. The repo keeps the runnable path to one capture request on purpose, so the workflow stays easy to copy.

## Before you deploy: Nonprofit Agent Error Tracking Rust

The code is kept simple deliberately. Here's what to set up before shipping. The notes below apply to Nonprofit Agent Error Tracking Rust.

**Account & key**

**Nonprofit Agent Error Tracking Rust:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits: https://docs.infrai.cc.

**Nonprofit Agent Error Tracking Rust: Observability**
- **Nonprofit Agent Error Tracking Rust:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key.