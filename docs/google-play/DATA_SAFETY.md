# Google Play Data safety draft

This document is a form worksheet. A person must compare it with the current
Google Play form and approve each answer before submission.

## Summary answers

- Does the application collect user data under the Google Play form definition: Use
  **Yes** for the draft. The application sends user data off the device to the server that
  the user selects. Google defines collection to include data that goes to a third-party
  server. A Console owner can use an applicable form exception only after legal approval.
- Does the application share user data: Use **No** for the draft only if the Console owner
  confirms the user-initiated transfer exception. The user selects the server and expects
  the application to send requests to that server.
- Does the application contain advertisements: No.
- Does the application provide account deletion: Yes. Use the selected server
  in the application or follow the public deletion instructions.
- Is data encrypted in transit: Yes by default. A user can approve a plain
  connection only to a private-network address.

The application sends requests only to servers that the user adds. Independent
server operators receive and control those requests. Catalyst Community does not
operate those servers and does not receive their data. The public demonstration
server is a separate service and does not require a guest to sign in.

The current Google form defines collection as transmission off the device. Do not use
"developer receives no data" as the reason for a **No data collected** answer.

## Data on a selected server

When a user signs in or uses a server function, the selected server can store:

| Data | Purpose | Account deletion result|
|---|---|---|
| LinkKeys subject, handle, display name, and role | Account management | Deleted |
| Session token hash and expiry | Authentication and security | Deleted |
| Audiobook position per track | Application function | Deleted |
| Private playlists | Application function | Deleted |
| Public playlists | Application function and user content | Deleted |
| Shared device registrations | Application function | Deleted |
| Blocked-account relationships | Safety and application function | Deleted |
| Reports submitted by the account | Safety and compliance | Deleted |
| Reports that target the account | Safety and compliance | Deleted |
| IP address in server logs | Security and operation | The server operator sets retention |

## Data on the device

The Android client stores server URLs, pinned key fingerprints, terms
acceptance, interface preferences, and session tokens. It protects session
tokens with the Android Keystore. It does not create a persistent media cache.

## Review questions

Before submission, a person must map the current form fields to account information,
application activity, user content, and device or other identifiers. Confirm each purpose,
retention rule, optional status, and sharing exception. Keep the form consistent with the
published privacy policy. Do not submit a "No data collected" answer unless Google or
qualified counsel confirms the exception for this exact client design.
