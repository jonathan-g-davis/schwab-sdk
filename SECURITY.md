# Security Policy

`schwab-sdk` mediates access to a real-money brokerage account. We take
reasonable steps to reduce the risk of credential or PII leakage through
this crate; nonetheless, the crate is provided "as is" under the MIT and
Apache-2.0 licences, both of which disclaim warranty and liability.
Responsibility for credential handling and for any leak that occurs in
caller code, in the host environment, or as a result of a defect in this
crate, rests with the user.

## Reporting a vulnerability

Please report suspected security issues privately, not via a public
GitHub issue or pull request, so we have time to investigate before
details become public.

- **Channel:** open a private advisory via
  [GitHub Security Advisories](https://github.com/jonathan-g-davis/schwab-sdk/security/advisories/new)
  on this repository.

## In scope

Defects in `schwab-sdk` itself that could plausibly cause:

- An access token, refresh token, customer id, account number, or
  account hash to be exposed outside this crate's intended boundary
  (e.g. printed via `Debug`, embedded in an `Error` variant, written to
  stdout/stderr by code in this crate).
- A bearer credential to be transmitted over plaintext as a result of
  this crate's transport selection (HTTPS / WSS) being bypassed.
- A request or frame to be sent to a host other than the one the caller
  configured.
- Memory-safety or panic-based denial-of-service in code reachable from
  the public API on well-formed or maliciously-shaped Schwab responses.

## Out of scope

- Vulnerabilities in Schwab's API itself, in caller-supplied
  `TokenProvider` implementations, or in downstream applications that
  use this crate. Please route these to the appropriate party.
- Credential leakage caused by caller code calling `.expose_secret()`
  and copying the result into a `String`, a log line, an error variant,
  or any other untyped context.
- Loss caused by trading activity, broker behaviour, or any cause other
  than a defect in this crate.
- Dependency advisories already covered by `cargo audit` against the
  versions pinned in `Cargo.toml`; these will be updated at a standard
  cadence.

## Supported versions

Only the most recent minor version receives security updates. Users on
older minor versions should plan to upgrade.

## What this policy is not

This policy describes how we triage reports and what we consider in
scope. It does not extend or override the warranty disclaimer in the
MIT or Apache-2.0 licences under which this crate is distributed.
