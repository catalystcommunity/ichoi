# Google Play Data safety draft

This document is a form worksheet. A person must compare it with the current
Google Play form and approve each answer before submission.

## Summary answers

- Does the developer collect user data: No.
- Does the developer share user data: No.
- Does the application contain advertisements: No.
- Does the application provide account deletion: Yes. Use the selected server
  in the application or follow the public deletion instructions.
- Is data encrypted in transit: Yes by default. A user can approve a plain
  connection only to a private-network address.

The application sends requests only to servers that the user adds. Independent
server operators receive and control those requests. Catalyst Community does not
operate those servers and does not receive their data. The public demonstration
server is a separate service and does not require a guest to sign in.

## Data on a selected server

When a user signs in or uses a server function, the selected server can store:

| Data | Purpose | Account deletion result|
|---|---|---|
| LinkKeys subject, handle, display name, and role | Account management | Deleted |
| Session token hash and expiry | Authentication and security | Deleted |
| Audiobook position per track | Application function | Deleted |
| Private playlists | Application function | Deleted |
| Public playlists | Application function and user content | Owner link removed; playlist stays |
| Shared device registrations | Application function | Deleted |
| Reports submitted by the account | Safety and compliance | Deleted |
| Reports that target the account | Safety and compliance | Deleted |
| IP address in server logs | Security and operation | The server operator sets retention |

## Data on the device

The Android client stores server URLs, pinned key fingerprints, terms
acceptance, interface preferences, and session tokens. It protects session
tokens with the Android Keystore. It does not create a persistent media cache.

## Review questions

Before submission, a person must confirm how the current form classifies data
that a user sends to an independent server that the user selects. If Google
classifies this transfer as collection, disclose the applicable account,
application activity, user content, and device or other identifier categories.
Do not submit a "No data collected" answer until this classification is approved.
