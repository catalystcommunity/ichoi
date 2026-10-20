# Google Play compliance checklist for an Ichoi mobile client

This document maps the Google Play Developer Program Policies to an Android app that is a
client for any Ichoi instance. It records the design decisions that keep the app compliant.
It lists the work that the server, the client, and the store listing need.

Status: local work that does not need release inputs is complete. AND-05 remains blocked.
IOS-11 needs a macOS archive check. Store submission also has external
blockers.

Policy source: <https://play.google/developer-content-policy/> (checked 2026-09-15).
Google changes these policies often. Check the source before each store submission.

Related Google guidance:

- Account deletion: <https://support.google.com/googleplay/android-developer/answer/13327111>
- Target audience: <https://support.google.com/googleplay/android-developer/answer/9867159>
- User-generated content: <https://support.google.com/googleplay/android-developer/answer/9876937>
- Target API level: <https://support.google.com/googleplay/android-developer/answer/11926878>
- Play Console requirements: <https://support.google.com/googleplay/android-developer/answer/10788890>

Responsibility values in the policy tables:

| Value | Meaning |
|---|---|
| **OK** | Ichoi complies now. No work is needed. |
| **CLIENT** | The mobile client must do this. |
| **SERVER** | The server needs a change. |
| **LISTING** | The Play Console listing or a Play Console form needs this. |
| **N/A** | The policy does not apply to this app. |

## 1. Design decisions

These decisions are fixed. Do not change them without a review of this document.

1. **The mobile clients do no server file administration.** The clients do not import
   files, do not export files, and do not copy media between servers. The clients do
   not call `export-manifest`, `export-chunk`, `begin-import`, `import-chunk`,
   `finish-import`, `import-track`, or `cancel-import`. The client has no "download" or
   "save to device" action. Playback cache in memory is permitted. A persistent file cache
   that the user can open outside the app is not permitted.
2. **Account deletion is a server operation with two callers.** A user can delete their
   own account. An admin can delete any account that is not the last admin. Both paths
   require a strong confirmation. Deletion removes the account row, all sessions, all
   audiobook progress, and all playlists that the account owns.
3. **The demo server is login-less for guests.** One admin account and one store-review
   member account exist. All other visitors are guests and do not sign in. Use exact
   LinkKeys identities for both accounts. The demo library contains only music and
   audiobooks that are in the public domain or under a licence that permits public
   streaming.
4. **The client sends user data only to servers that the user adds.** The developer
   receives no data. The client has no analytics, no crash reporter that sends data
   off-device, and no advertising SDK.
5. **The mobile clients are native applications.** The Android client uses Kotlin and lives
   in `android/`. The iOS client uses Swift and lives in `ios/`. Both clients use the
   permanent store ID `com.catalystsquad.ichoi`. Debug builds use
   `com.catalystsquad.ichoi.dev`. Do not change the store ID after publication.
6. **Reports have two target types.** A report identifies either a playlist or an account.
   It has a reason, optional details, an open, resolved, or dismissed state, and creation
   and resolution times. Account deletion removes reports that the account submitted and
   reports whose target is that account.
7. **Guest playlist writes stop when the first account exists.** A guest can write a public
   playlist only while the server has no accounts and the access mode admits that guest.
   After an account exists, a playlist write requires a session. A member can create only a
   private playlist. An administrator can create a public or private playlist. The server
   derives the owner from the session. It never accepts an owner from the request body.
   There is no guest-playlist override setting.
8. **The mobile clients are free and open source.** They use the Apache-2.0 licence. They
   have no purchase, subscription, donation, or advertising function. The initial Google
   Play target audience is ages 18 and over.
9. **The project website is a separate Pysocha site.** It lives in `website/`. It explains
   Ichoi, documents setup, and hosts the privacy policy, terms, and account-deletion help.
   Its source lives in `website/site-src/`. Pysocha generates `website/site/`; do not edit
   that generated directory by hand.
10. **A user can block an account.** A block applies on one selected server. The server
   hides public playlists from blocked accounts for that user. The server does not expose
   blocked account names through playlist results. A user can list and remove blocks.

## 2. Checklist by policy area

### 2.1 Intellectual Property

| Item | Status | Notes |
|---|---|---|
| App streams only from the user's own server | **OK** | Ichoi serves files from a library that the server operator owns. Nothing comes from the internet. |
| No "download a local copy" action | **CLIENT** | Decision 1. The raw-file endpoint in `server/crates/ichoi/src/server/media_http.rs` makes this trivial to add. Do not add it. |
| No cross-server copy in the client | **CLIENT** | Decision 1. Keep the words "copy", "transfer", and "download" out of the listing and the screenshots. |
| Cover art source | **OK** | The server fetches missing art from the Cover Art Archive and stores it next to the album. The client only displays what the server sends. |
| Trademarks in the listing | **LISTING** | Do not use "Subsonic", "Plex", "Spotify", or other marks in the title, description, or screenshots. Do not use the Android robot in the icon. |

### 2.2 Privacy: User Data

| Item | Status | Notes |
|---|---|---|
| Privacy policy URL in Play Console | **LISTING** | Required even if the app collects nothing. Host it on a public, non-PDF, stable URL. |
| Privacy policy link in the app | **CLIENT** | Put it in Settings and in the first-run screen. |
| Data safety form | **LISTING** | Declare: the app sends data only to user-configured servers. The developer collects nothing. See section 3 for the data that a server stores. |
| Encryption in transit | **CLIENT** | Default to the TLS CSIL listener (port 4043) with core key pinning, or HTTPS through the reverse proxy. Permit plain HTTP only after an explicit user choice, and only for private network addresses. Disclose this in the privacy policy. |
| Account creation implies account deletion | **SERVER** | A LinkKeys login auto-creates a member account. Add a server API operation and expose it in the Android, iOS, and browser clients. Decision 2 defines the deletion behavior. |
| Web URL for deletion requests | **LISTING** | Use the project account-deletion help page. It must direct the user to the selected server's browser deletion path or administrator. It must state that the server operator controls the account. |
| Prominent disclosure and consent | **N/A** | The app does not collect data outside the user's reasonable expectation. If satellite mode is added, show a one-time in-app notice that other users can start playback on this device. |
| Third-party SDKs | **CLIENT** | Use none that send data off-device. Decision 4. |

### 2.3 Privacy: Permissions

| Item | Status | Notes |
|---|---|---|
| Foreground service for background audio | **CLIENT** | Declare `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_MEDIA_PLAYBACK`. Set the service type to `mediaPlayback`. No Play declaration form is required for this type. |
| Storage permissions | **CLIENT** | Request none. Decision 1 removes every reason to touch storage. Never request `MANAGE_EXTERNAL_STORAGE`. |
| Network permissions | **CLIENT** | `INTERNET` and `ACCESS_NETWORK_STATE` only. |
| Local network discovery | **CLIENT** | If you add mDNS discovery of servers, use `NsdManager`. It needs no runtime permission. |
| Bluetooth | **CLIENT** | Do not request it. Android routes audio to Bluetooth outputs without app permission. |
| `QUERY_ALL_PACKAGES` | **CLIENT** | Do not request it. |

### 2.4 Privacy: Device and Network Abuse

| Item | Status | Notes |
|---|---|---|
| No self-update outside Google Play | **CLIENT** | Do not add an in-app updater. |
| No download of executable code | **CLIENT** | The native clients use the generated Kotlin and Swift bindings and download no code. |
| No proxy or VPN function | **OK** | Ichoi has none. |

### 2.5 Privacy: Deceptive Behavior and Misrepresentation

| Item | Status | Notes |
|---|---|---|
| Accurate description | **LISTING** | State clearly that the app needs an Ichoi server. Say who runs the server. Do not imply that the app includes music. |
| Screenshots show real function | **LISTING** | Take screenshots against the demo server. |

### 2.6 Monetization and Ads

| Item | Status | Notes |
|---|---|---|
| Free app, no in-app purchases | **OK** | No billing obligations. |
| Paid app | **N/A** | Decision 8 fixes the price at free. |
| Donations | **N/A** | Decision 8 excludes an in-app donation function. |
| Ads | **N/A** | None. |

### 2.7 User Generated Content

Ichoi has user content: playlist names, display names, and playlists visible to other
users. The policy covers "clients to direct users to a UGC platform". The exposure is low.
The server operator controls the user set. The measures below are the defensible minimum.

| Item | Status | Notes |
|---|---|---|
| Terms of use before content creation | **CLIENT** | Show a short terms screen on first run. Require a tap to accept. |
| Public-content filter | **SERVER** | After the first account exists, only an administrator can publish a public playlist. Members can create private playlists. |
| Report action | **CLIENT** | Add "Report playlist" and "Report user" actions that send a message to the server admin. |
| Report delivery | **SERVER** | Add report submission, admin listing, and admin resolution operations. Store the target type, target ID, reason, optional details, status, reporter, and times. Show open reports in the admin Settings view. |
| Admin removal of playlists | **SERVER** | There is no delete-playlist operation. Add one for the owner and for admins. |
| Block action | **SERVER** and **CLIENT** | Public playlists are publicly accessible user content. Add account blocking to the server and all clients. |
| Content rating questionnaire | **LISTING** | Answer "yes" to user interaction and to sharing of user-created content. Record the assigned content rating. The target audience remains ages 18 and over. |

### 2.8 Restricted Content, Families, Impersonation, Malware

| Item | Status | Notes |
|---|---|---|
| Restricted content | **OK** | The app shows what the server has. The server operator is responsible for the library. |
| Families program | **LISTING** | Do not opt in. Set the initial target audience to ages 18 and over. |
| Impersonation | **OK** | Publish under the Catalyst Community developer account. |
| Malware and unwanted software | **OK** | None of the listed behaviors apply. |

### 2.9 Functionality and reviewer access

| Item | Status | Notes |
|---|---|---|
| App works without a server | **CLIENT** | Show a first-run screen that explains what Ichoi is and offers "Connect to the demo server". A reviewer must reach content in two taps. |
| App access instructions | **LISTING** | Give the guest two-tap path and the private store-review member credential. Include steps for each sign-in-only function. |
| Stability | **CLIENT** | Test on Android 10 through the current release. Crashes on the reviewer's device are a policy violation. |
| Target API level | **OK** | The client targets API level 36. Google requires API level 36 for new apps and updates from August 31, 2026. Recheck this value before submission. |

## 3. Data that a server stores about a user

Use this list for the privacy policy and for the Data safety form. All of it stays on the
server that the user chose. None of it reaches the developer.

| Data | Where | Deleted by account deletion |
|---|---|---|
| LinkKeys subject `uuid@domain` | `accounts.id` | Yes |
| Handle and display name | `accounts` | Yes |
| Role | `accounts` | Yes |
| Session token hash and expiry | `sessions` | Yes |
| Audiobook position per track | `audiobook_progress` | Yes |
| Private playlists | `playlists`, `playlists/*.m3u` | Yes |
| Public playlists | `playlists`, `playlists/*.m3u` | Yes |
| Shared device registrations | `players` | Yes |
| Blocked-account relationships | account-block storage | Yes |
| Content reports submitted by the account | `content_reports` | Yes |
| Reports whose target is the account | `content_reports` | Yes |
| IP address in server logs | operator log retention | Operator policy |

The clients store on the device: server URLs, pinned core key fingerprints, session tokens,
theme and language preference. Keep Android session tokens in the Android Keystore. Keep
iOS session tokens in the Keychain.

## 4. Demo server

### 4.1 What works today

The configuration below is possible with the current server. Guests browse and play without
a login. One LinkKeys identity is the admin.

```sh
ICHOI_ACCESS_MODE=open
ICHOI_ADMIN_TOKEN=<one-time bootstrap token>
ICHOI_PUBLIC_URL=https://demo.ichoi.invalid
ICHOI_LINKKEYS_RP=true
ICHOI_LINKKEYS_RP_ADDR=linkkeys.ichoi.invalid:4987
ICHOI_LINKKEYS_RP_FINGERPRINTS=<rp fingerprints>
ICHOI_LINKKEYS_RP_API_KEY_FILE=/run/secrets/linkkeys_rp_api_key
ICHOI_LINKKEYS_RP_DOMAIN=demo.ichoi.invalid
ICHOI_LINKKEYS_TRUSTED_IDENTITIES=<admin-handle>@<admin-domain>,<reviewer-handle>@<reviewer-domain>
```

DNS-less local RP mode also works over public HTTPS. The login handler only requires that
the `Origin` header matches the `Host` header, which a reverse proxy preserves.

Use exact `handle@domain` values in the trusted identities. A bare domain would let every
verified handle at that domain create a member account on the demo server. Keep the
reviewer credentials outside the repository. A new login recreates the trusted reviewer
account after an account-deletion test.

Once the admin account exists, the server treats every anonymous visitor as follows:

| Action | Anonymous guest on the demo server | Source |
|---|---|---|
| Browse, search, play | Permitted | open access mode |
| `whoami` | 401; the web UI stays a nameless guest | `handlers/mod.rs` SessionService |
| Save audiobook progress | Denied | `handlers/mod.rs` progress account resolution |
| Export or import media | Denied | `require_export_or_guest_instance`, `require_admin_or_guest_instance` |
| Share a device as a jukebox target | Denied | `enable_share` |
| Control an existing shared target | Denied for anonymous users after the first account; signed-in users or the owning node only | `handlers/mod.rs` `PlayerService::control` |
| Save the queue as a public playlist | Denied after the first account exists | `save_queue_playlist` in `server/http.rs` |

### 4.2 Closed gap: anonymous playlist writes

The `save_queue_playlist` HTTP handler now requires a signed-in session after the first
account exists. It derives the playlist owner from that session. It does not accept an owner
in the request body. A test covers the public demo guest restrictions.

### 4.3 Demo library

- Use only recordings whose licence permits public streaming. Musopen, the Free Music
  Archive, and LibriVox are possible sources. A person must verify each recording, licence,
  licence URL, and attribution. A source name does not approve a recording.
- Keep a `LICENSES.md` in the demo music directory that lists every recording and its
  licence. A reviewer or a rights holder can ask for it.
- Do not register any jukebox outputs on the demo. The scratch container has none.
- Set a short session lifetime and rotate the admin token after the first login.

## 5. Task rules and sequence

Task states:

| State | Meaning |
|---|---|
| **DONE** | The implementation and all locally available acceptance checks are complete. |
| **READY** | The task has the decisions and inputs that an agent needs. |
| **BLOCKED** | The listed input must be fixed before work starts. |
| **EXTERNAL** | A person must use an external account, approve content, or change production. |
| **LATER** | The task is not required for the first Google Play submission. |

Use these rules for every task below:

1. Give one task to one agent. The agent must change only the listed area.
2. Do not run two server tasks at the same time. The server handlers and store are shared
   files.
3. Run each CSIL schema task alone. Run `./tools.sh gen` after a schema change. This
   command regenerates the Rust server bindings and every client binding. Never edit
   generated code by hand.
4. Run `./tools.sh check` after each server task. Run the smallest relevant test first.
5. A task is complete only when all acceptance checks pass. An explicit external-value
   placeholder is permitted only when a release or deployment guard rejects it.
6. External tasks do not authorize an agent to create accounts, publish an application,
   deploy production, handle production secrets, or approve legal or licence claims.

### 5.1 Plan corrections

Reviews found and corrected these plan defects:

- The demo table now states the existing anonymous shared-target restriction. The prior
  text described access that the server did not permit.
- SRV-02 now names the repository migration directory. The prior area did not name it.
- SRV-08 now depends on report storage because playlist deletion removes target reports.
- WEB-01 now owns the browser terms gate because the browser can create playlists.
- AND-10 and IOS-08 include locally available checks. Permanent IDs, store tools, Xcode,
  and devices remain separate blockers.
- PLAY-07 permits a guarded `.invalid` draft before the production demo exists.
- Missing public values use reserved `.invalid` placeholders. Guards prevent release or
  deployment with those values.
- The completion rule permits only guarded placeholders for external values. The prior rule
  conflicted with the required development IDs, domains, and contacts.
- The demo-library text no longer calls a source safe. A person must approve each recording
  and its exact licence.
- The 2026-09-15 policy review found that public user content needs an in-app block
  function. The prior plan incorrectly marked blocking as not applicable.
- The Apple deletion rules require the app to delete shared user content with the account.
  SRV-11 corrects the prior public-playlist retention rule for both mobile clients.
- Google requires new apps and updates to target API level 36 from August 31, 2026. The
  Android project already targets API level 36.

## 6. Server tasks

Run `SRV-01` through `SRV-13` in order. Each task must leave the server build and tests
complete.

### SRV-01: Protect playlist writes

**State:** DONE

**Area:** `server/crates/ichoi/src/server/http.rs`, its HTTP tests, and the browser request
body in `server/web/themes/default/src/stores/playback.tsx`.

**Work:** Remove `owner` from the request. Derive the owner from the authenticated session.
Permit an admitted guest only when the server has zero accounts. Require a session when at
least one account exists. A guest playlist is always public.

**Acceptance:** Tests cover a signed-in member, a zero-account guest, a guest after the first
account exists, an invalid session, a private member playlist, and an attempt to supply a
false owner. Run the web tests and build. Run `./tools.sh check`.

### SRV-02: Store content reports

**State:** DONE

**Area:** `server/migrations/<timestamp>_content_reports/{up.sql,down.sql}`,
`server/crates/ichoi/src/db/{models.rs,schema.rs,store.rs}`, and database tests.

**Work:** Add `content_reports`. Store an ID, reporter account ID, target type, target ID,
reason, optional details, status, creation time, and optional resolution time. Permit target
types `playlist` and `account`. Permit statuses `open`, `resolved`, and `dismissed`.
Delete reports submitted by an account when that account is deleted.

**Acceptance:** Store tests cover insert, pagination, status update, reporter deletion, and
invalid values. The down migration restores the previous schema. Run `./tools.sh check`.

### SRV-03: Add report submission

**State:** DONE

**Depends on:** SRV-02

**Area:** `server/schema/`, regenerated bindings, the report handler, and service tests.

**Work:** Add `report-content`. The request contains the target type, target ID, reason, and
optional details. Reasons are `objectionable-content`, `harassment`, `spam`, and
`other`. Require a signed-in account. Verify that the target exists. Limit details to 2,000
Unicode scalar values. Return the stored report.

**Acceptance:** Tests cover each target type, each reason, missing targets, anonymous calls,
the details limit, and successful storage. Run `./tools.sh gen` twice and verify that the
second run changes no files. Run `./tools.sh check`.

### SRV-04: Add report administration

**State:** DONE

**Depends on:** SRV-03

**Area:** `server/schema/`, regenerated bindings, the admin handler, and service tests.

**Work:** Add paginated report listing and a report-status update operation. Only an admin
can call them. A status update records the resolution time for `resolved` or `dismissed`
and clears it for `open`.

**Acceptance:** Tests cover pagination, all status changes, a missing report, a member call,
and an anonymous call. Run `./tools.sh gen` twice and verify that the second run changes no
files. Run `./tools.sh check`.

### SRV-05: Implement account deletion storage

**State:** DONE

**Depends on:** SRV-02

**Area:** a handwritten account-deletion module, `server/crates/ichoi/src/db/store.rs`, any
required migration, and focused tests.

**Work:** Implement one deletion function for both API callers. It deletes sessions,
audiobook progress, shared device registrations, reports submitted by the account, reports
whose target is the account, and private playlists. It removes the owner from public
playlists. It refuses to delete the last admin.

Private playlist files need crash-safe deletion. Move them to a private staging directory on
the same filesystem before the database transaction. Restore them if the transaction fails.
After a commit, delete the staged files. Write enough staging metadata to identify the
original playlist row and path. At startup, restore a staged file if its database row still
exists. Delete the staged file if its database row does not exist. Validate every path
before a file operation and keep it inside the configured playlist directory.

**Acceptance:** Tests cover every row type in section 3, private file deletion, public file
retention, a missing private file, a file error, database rollback, stale staging cleanup,
path escape attempts, and last-admin protection. Run `./tools.sh check`.

SRV-11 replaces the public-playlist behavior and its related acceptance points. The final
server deletes every public and private playlist that the account owns.

### SRV-06: Add self account deletion

**State:** DONE

**Depends on:** SRV-05

**Area:** `server/schema/session.csil`, regenerated bindings, the session handler, and
service tests.

**Work:** Add `delete-account` to `SessionService`. Require a session and an exact copy of
the current account handle as confirmation. Call the shared deletion function.

**Acceptance:** Tests cover success, a wrong confirmation value, an anonymous call, the last
admin, and a second request for the deleted account. A successful request invalidates every
session for that account. Run `./tools.sh gen` twice and verify that the second run changes
no files. Run `./tools.sh check`.

### SRV-07: Add admin account deletion

**State:** DONE

**Depends on:** SRV-06

**Area:** `server/schema/admin.csil`, regenerated bindings, the admin handler, and service
tests.

**Work:** Add an admin account-deletion operation. Require the target account ID and an exact
copy of the target handle. Call the shared deletion function.

**Acceptance:** Tests cover success, a member call, an anonymous call, a missing account, a
wrong confirmation value, deletion of another admin, and last-admin protection. Run
`./tools.sh gen` twice and verify that the second run changes no files. Run
`./tools.sh check`.

### SRV-08: Add playlist deletion

**State:** DONE

**Depends on:** SRV-01 and SRV-02

**Area:** `server/schema/library.csil`, regenerated bindings, playlist storage and handlers,
and service tests.

**Work:** Add `delete-playlist`. Permit the owner or an admin. Use the same safe
file-staging method as account deletion. Delete reports whose target is the playlist.

**Acceptance:** Tests cover owner deletion, admin deletion, a different member, an anonymous
call, a missing playlist, file failure, database rollback, and path escape attempts. Run
`./tools.sh gen` twice and verify that the second run changes no files. Run
`./tools.sh check`.

### SRV-09: Make the session lifetime configurable

**State:** DONE

**Area:** server configuration, session creation, deployment documentation, and focused
tests.

**Work:** Add the TOML key `session_lifetime_hours` and the environment variable
`ICHOI_SESSION_LIFETIME_HOURS`. Keep 720 hours as the default. Reject zero and invalid
values. Document a 24-hour value for the demo server.

**Acceptance:** Tests cover the default, the TOML value, the environment override, zero, an
invalid value, and the expiry time of a new session. Run `./tools.sh check`.

### SRV-10: Verify the complete server surface

**State:** DONE

**Depends on:** SRV-01 through SRV-09

**Area:** integration tests and documentation only. Do not change behavior to make a test
pass without returning the failed task to its owner.

**Work:** Add one end-to-end test for playlist reporting, admin review, playlist deletion,
and account deletion. Verify generated-tree reproducibility.

**Acceptance:** Two consecutive `./tools.sh gen` runs produce the same tree. `./tools.sh
check` passes.

### SRV-11: Delete all playlists with an account

**State:** DONE

**Depends on:** SRV-05 and SRV-08

**Area:** the handwritten account-deletion module, playlist storage, report storage, and
focused tests.

**Work:** Delete each public and private playlist that the account owns. Use the existing
file-staging method. Delete reports that target those playlists. Do not leave an ownerless
public playlist. This task replaces the public-playlist retention rule in SRV-05.

**Acceptance:** Tests cover public and private playlist rows, playlist files, reports that
target each playlist, file failure, database rollback, stale staging cleanup, and path
escape attempts. Run `./tools.sh check`.

### SRV-12: Add account blocking

**State:** DONE

**Depends on:** SRV-11

**Area:** a migration, `server/schema/`, regenerated bindings, playlist queries, handlers,
and focused tests.

**Work:** Add block, unblock, and list-blocked-account operations. Require a signed-in
account. Reject a self-block. Hide public playlists from a blocked owner in browse and
search results for that account. Delete block rows when either account is deleted.

**Acceptance:** Tests cover block, repeated block, unblock, repeated unblock, self-block,
anonymous calls, account deletion, pagination, and playlist filtering. Run `./tools.sh gen`
twice and verify that the second run changes no files. Run `./tools.sh check`.

### SRV-13: Limit public playlist publication

**State:** DONE

**Depends on:** SRV-12

**Area:** the queue-playlist HTTP handler and its tests.

**Work:** Permit only an administrator to create a public playlist after the first account
exists. Force a member playlist to private or reject an explicit public request. Keep the
zero-account guest bootstrap behavior. This administrator gate is the publication filter
for user content.

**Acceptance:** Tests cover an administrator public playlist, an administrator private
playlist, a member private playlist, a member public request, a zero-account guest, a guest
after the first account exists, and a false owner. Run the web tests and `./tools.sh check`.

## 7. Browser client tasks

Run these tasks in order because they share routes, services, and locale files.

### WEB-01: Add self account deletion

**State:** DONE

**Depends on:** SRV-06

Add first-run terms acceptance and a strong account-deletion confirmation dialog in Settings.
The client must store the terms acceptance and must block playlist creation until the user
accepts the terms. The user must enter the current handle to delete an account. Clear the
local session only after deletion succeeds. On failure, keep the session and tell the user
to contact the administrator of the selected server. Show the server URL. Test terms for
signed-in users and guests. Test deletion success, validation, and service failure. Run
`npm test` and `npm run build` in the web theme.

### WEB-02: Add report actions

**State:** DONE

**Depends on:** SRV-03

Add **Report playlist** and **Report user** actions. Add the four reasons and optional
details. Do not expose report actions without a signed-in session. Test request construction,
validation, success, and failure. Run the web tests and build.

### WEB-03: Add the admin report queue

**State:** DONE

**Depends on:** SRV-04, SRV-07, and SRV-08

Add an admin-only report section in Settings. Show open reports first. Link each live target
to the relevant admin action. Add resolve and dismiss controls. Test access control, empty
state, pagination, target deletion, resolve, and dismiss. Run the web tests and build.

### WEB-04: Add account blocking

**State:** DONE

**Depends on:** SRV-12

Add **Block user** beside the existing report action. Add a blocked-account list in
Settings with an **Unblock** action. Explain that a block hides public playlists from that
account on the selected server. Test block, unblock, filtering, failure, and profile
separation. Run the web tests and build.

## 8. Android client tasks

The Android client uses Kotlin in `android/`. Its minimum supported release is Android 10.
Use the generated Kotlin client as an input. Do not edit that generated client by hand.

### AND-00: Select the permanent application ID

**State:** DONE

The permanent Android application ID is `com.catalystsquad.ichoi`. It cannot change after
publication. The Play Console application uses this ID. Debug builds add the `.dev` suffix.
The `verifyStoreRelease` task rejects a release build with a different ID.

### AND-01: Create the Android project

**State:** DONE

Create a reproducible Kotlin project in `android/`. Set the minimum SDK to Android 10. Set
the compile and target SDK to the versions required at implementation time. Add unit, lint,
and debug-build commands. Use a clearly marked development-only application ID in one build
configuration location. Prevent a store release while that ID is active. Do not add
analytics, advertising, billing, crash reporting, storage, Bluetooth, or package-query
dependencies or permissions.

### AND-02: Add server profiles and secure sessions

**State:** DONE

**Depends on:** AND-01

Integrate the generated Kotlin binding. Store server URLs and pinned key fingerprints in app
preferences. Store session tokens through the Android Keystore. Test profile separation,
token replacement, sign-out, and token removal.

### AND-03: Enforce connection security

**State:** DONE

**Depends on:** AND-02

Use pinned CSIL TLS or HTTPS by default. Permit plain HTTP only for a private address after
an explicit warning and confirmation. Reject public plain-HTTP addresses. Test IPv4, IPv6,
redirects, pin mismatch, and user cancellation.

### AND-04: Add first-run and server connection

**State:** DONE

**Depends on:** AND-03, SITE-03, and SITE-05

Explain Ichoi, require acceptance of the terms, link the privacy policy, and provide **Add my
server**. Keep the demo action hidden until a demo URL exists. Test first acceptance,
repeated launch, link handling, validation, and connection failure.

### AND-05: Add the demo connection

**State:** BLOCKED

**Depends on:** AND-04 and DEMO-04

Add **Connect to demo server** with the fixed production URL. A reviewer must reach playable
content in two taps. Test the default profile and offline failure.

### AND-06: Add playback and the foreground service

**State:** DONE

**Depends on:** AND-03

Implement local playback and the media notification. Declare only `FOREGROUND_SERVICE`,
`FOREGROUND_SERVICE_MEDIA_PLAYBACK`, `INTERNET`, and `ACCESS_NETWORK_STATE`. Set the
service type to `mediaPlayback`. Do not write media to shared or app-private persistent
storage.

### AND-07: Add account settings

**State:** DONE

**Depends on:** AND-02 and SITE-03

Add the privacy link and sign-out. Test link handling and local token removal.

### AND-08: Add account deletion

**State:** DONE

**Depends on:** AND-07 and SRV-06

Require the current handle as confirmation. Clear the token only after success. On failure,
keep the session and tell the user to contact the selected server administrator. Show the
server URL. Do not claim success after a timeout or an unknown response.

### AND-09: Add report actions

**State:** DONE

**Depends on:** AND-02 and SRV-03

Add report actions for playlists and accounts. Add the four reasons and optional details.
Test target IDs, validation, success, and service failure.

### AND-10: Add Android compliance gates

**State:** DONE

**Depends on:** AND-01 through AND-04, AND-06 through AND-09

Fail CI if the manifest contains storage, Bluetooth, or package-query permissions. Audit the
dependency graph for analytics, advertising, billing, updater, and remote-code SDKs. Run
unit tests, lint, and the release build. Check non-generated client source for calls to the
prohibited operations in decision 1. Verify that playback creates no persistent media file.
Record tests on Android 10 through the current Android release.

### AND-11: Add account blocking

**State:** DONE

**Depends on:** AND-09 and SRV-12

Add **Block user** beside **Report user**. Add a blocked-account list in Settings with an
**Unblock** action. Keep blocks separate for each server profile. Test block, unblock,
filtering, failure, and profile separation.

### AND-12: Add the launcher icon

**State:** DONE

**Depends on:** ART-00

The launcher icon comes from `assets/brand/apps/android/res/`. The manifest uses
`@mipmap/ic_launcher`. The adaptive icon has a monochrome layer for themed icons.
Device checks of the icon stay external.

Add the approved adaptive and legacy launcher-icon resources. Add the icon to the
application manifest. Verify the icon on Android 10 and the current Android release. Do not
use the Google Play store-listing icon as a launcher resource without the required adaptive
icon layers.

Store-release verification stays blocked until AND-05 is complete.
Physical-device checks stay external. `android/PHYSICAL_DEVICE_TESTS.md` gives the required
Android 10 through current-release test record.

## 9. iOS client tasks

The iOS client uses Swift in `ios/`. It follows the same protocol, privacy, deletion, and
reporting decisions. Apple store policy and submission work need a separate compliance
document.

| ID | State | Depends on | Result |
|---|---|---|---|
| IOS-00 | **DONE** | none | The permanent bundle ID is `com.catalystsquad.ichoi`. The release guard rejects every other Release bundle ID. |
| IOS-01 | **DONE** | none | The Swift project uses one development-only bundle ID, has a release guard, and references generated Swift without editing it. |
| IOS-02 | **DONE** | IOS-01 | Server profiles, TLS pinning, downgrade rejection, and private-address HTTP consent are present. |
| IOS-03 | **DONE** | IOS-02 | LinkKeys sessions use the allow-listed native callback and store tokens by profile in the Keychain. |
| IOS-04 | **DONE** | IOS-03, SITE-03, SITE-05 | First-run terms, legal links, profile selection, and server connection are present. |
| IOS-05 | **DONE** | IOS-02 | In-memory local playback and supported background audio are present. |
| IOS-06 | **DONE** | IOS-03, SRV-06 | Settings, sign-out, and failure-safe account deletion are present. |
| IOS-07 | **DONE** | IOS-03, SRV-03 | Signed-in users can report listed playlists and their owner accounts. |
| IOS-08 | **DONE** | IOS-01 through IOS-07 | Available static checks and tests are present. `docs/APP_STORE_COMPLIANCE.md` gives exact macOS, Xcode, device, signing, and App Store checks. Those checks stay external on Linux. |
| IOS-09 | **DONE** | IOS-08 | The app privacy manifest declares the app-only `UserDefaults` reason and no tracking. The static compliance check verifies the manifest. |
| IOS-10 | **DONE** | IOS-07, SRV-12 | Account block and unblock actions use the selected server profile. Server tests cover filtering. Client policy tests cover failure and profile separation. |
| IOS-11 | **READY** | ART-00 | `Ichoi/Assets.xcassets` contains the approved `AppIcon` set. Verify the archive asset validation on macOS. |

## 9.1 Store artwork task

### ART-00: Approve the application artwork

**State:** DONE

The approved artwork and its exports are in `assets/brand/`. `assets/brand/README.md`
tells which file to use for each platform.

Approve one Ichoi icon design and its licence. Supply source artwork that can produce the
iOS app icon, Android adaptive icon layers, a 512 by 512 pixel Google Play icon, and a 1024
by 500 pixel Google Play feature graphic. Do not use third-party marks or the Android robot.

## 10. Project website tasks

Create `website/` with the same Pysocha structure as `../tinku/website/`. Keep source in
`website/site-src/` and generated output in `website/site/`. Use a pinned Pysocha commit.
The container image must rebuild from source. A second local build must change no files.

### SITE-01: Create the website project

**State:** DONE

Create the Pysocha configuration, task runner, templates, style source, generated output,
Dockerfile, Caddyfile, and local build instructions. Use relative internal links. Use the
reserved placeholder domain `ichoi.invalid`. The deployment guard must reject this value.
Verify two reproducible builds.

### SITE-02: Write the product and setup pages

**State:** DONE

**Depends on:** SITE-01

Write separate pages for the product explanation, server setup, Android setup, iOS setup,
and server-operator responsibilities. State that users supply their own server and media.
Do not imply that the clients include music.

### SITE-03: Write the privacy policy

**State:** DONE

**Depends on:** SITE-01

Use sections 1 and 3 as the data inventory. Identify the application and developer. State
that data goes only to servers that the user selects. State that each server operator sets
log retention and controls server data. Include a privacy contact method placeholder.

### SITE-04: Write account-deletion help

**State:** DONE

**Depends on:** SITE-01 and WEB-01

Explain how to open the selected server in a browser, sign in, and use **Settings → Delete
account**. Tell users to contact that server's administrator if deletion fails or the server
does not implement the operation. State that the Ichoi application developer cannot access
independently operated servers. Include a support contact placeholder.

### SITE-05: Write terms and reporting rules

**State:** DONE

**Depends on:** SITE-01

Define prohibited content and behavior. Explain the playlist and account report actions.
Explain that the selected server operator reviews reports and controls removal. Require
acceptance before a user creates a playlist.

### SITE-06: Approve contacts and claims

**State:** EXTERNAL

**Depends on:** SITE-02 through SITE-05 and SITE-08

A person must approve the privacy and support contacts, data claims, terms, and licence
claims. Remove all placeholders only after approval.

### SITE-07: Select a domain and deploy

**State:** EXTERNAL

**Depends on:** SITE-06

Select the domain. Update the deployment values. Use the Catalyst Community registry and
Kubernetes secrets. Publish the site and verify every public page without authentication or
geographic restriction.

### SITE-08: Update deletion and blocking text

**State:** DONE

**Depends on:** SRV-11 through SRV-13, WEB-04, AND-11, and IOS-10

State that account deletion removes all playlists that the account owns. Explain how a user
blocks and unblocks an account on one selected server. Keep the public contact method easy
to find. Rebuild the generated site twice and verify that the second build changes no files.

## 11. Demo server tasks

### DEMO-01: Add an anonymous-access smoke test

**State:** DONE

**Depends on:** SRV-01

Create a repeatable test that proves an anonymous visitor can browse and play but cannot
write playlists after the admin exists. It must also prove that import, export, sharing, and
progress writes fail. Do not duplicate the playlist authorization implementation.

### DEMO-02: Prepare deployment files

**State:** DONE

Prepare the Caddy and container configuration from section 4.1. Use the reserved placeholder
host `demo.ichoi.invalid`. The deployment guard must reject this value. Do not add production
secrets.

### DEMO-03: Approve the demo library

**State:** EXTERNAL

Select recordings whose licences permit public streaming. Record the source, recording,
author, licence, licence URL, and attribution in `LICENSES.md`. A person must approve the
licence inventory before deployment.

### DEMO-04: Select the URL, deploy, and bootstrap

**State:** EXTERNAL

**Depends on:** DEMO-01 through DEMO-03 and SRV-09

Select the demo URL. Deploy one core. Set exact admin and store-review `handle@domain`
values as the only trusted identities. Bootstrap the admin once, remove the bootstrap
token, set a short session lifetime, and register no jukebox outputs. Store the reviewer
credential only in the store review records.

### DEMO-05: Verify the public demo

**State:** EXTERNAL

**Depends on:** DEMO-04

Run the smoke test from a public network. Save the date, server version, test result, and
reviewer steps as release evidence. Test guest browsing and the reviewer sign-in, report,
block, unblock, and account-deletion flows. Sign in again after deletion to restore the
trusted reviewer member account.

## 12. Google Play tasks

Keep drafts in the repository. A person with Play Console access performs every Console
change.

| ID | State | Depends on | Result |
|---|---|---|---|
| PLAY-01 | **EXTERNAL** | AND-00 | Verify the developer identity, create the Play application, and register the permanent package name. |
| PLAY-02 | **DONE** | SITE-02 | Draft listing text that states the server requirement, free price, Apache-2.0 licence, and absence of bundled music. Do not use third-party marks. |
| PLAY-03 | **DONE** | SITE-03, SRV-06 | Draft the Data safety answers from section 3. |
| PLAY-04 | **EXTERNAL** | SITE-07 | Enter the public privacy-policy and account-deletion URLs. |
| PLAY-05 | **EXTERNAL** | PLAY-01 | Set the target audience to ages 18 and over. Do not join the Families program. |
| PLAY-06 | **DONE** | SITE-05 | Draft the content-rating and UGC questionnaire answers. |
| PLAY-07 | **READY** | DEMO-04 | Update the App access draft with the production URL, reviewer LinkKeys identity, credential, and steps for every sign-in-only function. |
| PLAY-08 | **BLOCKED** | AND-05, DEMO-05 | Capture at least two accurate screenshots against the production demo. |
| PLAY-09 | **EXTERNAL** | SRV-11 through SRV-13, AND-10 through AND-12, SITE-08, PLAY-02 through PLAY-08, PLAY-10 | Confirm API level 36 or the newer required level, complete the Console forms, upload the bundle, and submit it for review. |
| PLAY-10 | **EXTERNAL** | ART-00 | Upload `assets/brand/stores/google-play-icon-512.png` and `assets/brand/stores/google-play-feature-1024x500.png`. |
