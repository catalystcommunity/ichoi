# Ichoi iOS App Store compliance plan

Status: local work that does not need release inputs is complete. IOS-11 needs a
macOS archive check. App Store submission also has external blockers.

This plan uses simple English. The policy review date is 2026-09-15. Check the current
Apple rules before each submission. The project is free and uses the Apache-2.0 licence.
It has no ads, purchases, subscriptions, donations, analytics, remote crash service, or
third-party tracking SDK.

Policy sources:

- App Review Guidelines: <https://developer.apple.com/app-store/review/guidelines/>
- Account deletion: <https://developer.apple.com/support/offering-account-deletion-in-your-app/>
- Privacy manifests: <https://developer.apple.com/documentation/bundleresources/privacy-manifest-files>
- Submission requirements: <https://developer.apple.com/news/upcoming-requirements/>
- App Store Connect properties: <https://developer.apple.com/help/app-store-connect/reference/app-information/required-localizable-and-editable-properties>

## Product and legal data

- The app is a native SwiftUI client for an Ichoi server selected by the user.
- The app does not include music or audiobook files. The selected server provides them.
- The app stores server profiles on the device. It stores session tokens in the iOS
  Keychain, with one Keychain account for each server profile.
- Data goes only to the server that the user selects. The developer does not receive it.
- The privacy policy is at `https://catalystichoi.com/privacy.html`.
- The terms are at `https://catalystichoi.com/terms.html`.
- Account deletion help is at `https://catalystichoi.com/account-deletion.html`.

The website is not deployed yet. A release owner must deploy it, test each URL, and approve
the privacy policy, terms, and deletion text. This legal approval is external.

## Project gates

IOS-00 is done. The permanent bundle ID is `com.catalystsquad.ichoi`. It is in
`ios/Config/Release.xcconfig`. Do not change it after App Store publication. Register this
ID as an explicit App ID in the Apple Developer account before the first archive.

The Debug configuration uses `com.catalystsquad.ichoi.dev`. It is development-only.
The release build has a guard script. The guard rejects every Release bundle ID except the
permanent ID. The guard is at `ios/scripts/verify-release.sh`.

The Team ID is `W9N9XMS694`. It is in `ios/Config/Base.xcconfig`. The project uses
automatic signing. Xcode gets the distribution certificate and provisioning profile through
the App Store Connect API key. Do not put secrets in this repository.

The Reactorcide secret path `nsc/apple` holds the App Store Connect API key. It has these
keys: `asc_api_key_p8`, `asc_key_id`, `asc_issuer_id`, and `team_id`. The key must be a Team
key with the Admin role. An App Manager key cannot use cloud-managed distribution
certificates, and the export fails.

IOS-11: `Ichoi/Assets.xcassets` contains the approved `AppIcon` set from
`assets/brand/apps/apple/`. It has default, dark, and tinted appearances. Verify the asset
validation in a macOS archive.

The app supports iPhone only (`TARGETED_DEVICE_FAMILY = 1`). The project declares that it
uses no non-exempt encryption. The app uses only the standard HTTPS of the operating system.

## Privacy and network behavior

The app uses HTTPS by default. It checks the configured HTTPS certificate fingerprints. It
rejects an HTTPS redirect to HTTP and rejects a pin mismatch. It permits HTTP only when all
of these conditions are true:

1. The user turns on the private-network HTTP option.
2. The host is a private IPv4, private IPv6, or localhost address.
3. The user saves the server profile after the warning.

The app rejects public HTTP. It requests no location, contacts, photos, files, Bluetooth,
or advertising permissions. It uses the `audio` background mode for playback.

The app includes `PrivacyInfo.xcprivacy`. The manifest declares `UserDefaults` with reason
`CA92.1`. The app uses these preferences only for app data. The manifest declares no
tracking. Review the manifest again after each dependency or API change.

## User content and account deletion

Public playlists, playlist names, and account display names are user content. Apple
Guideline 1.2 requires filtering, reports, timely responses, account blocking, and a public
contact method. Reports, administrator removal, account blocking, and client block controls
exist. The server permits only administrators to publish public playlists after bootstrap.
The release owner must document the server moderation process and response owner before
submission.

Apple states that account deletion must also delete shared user content. The server deletes
all public and private playlists that the account owns, including their staged files and
reports that target those playlists.

## User flows to review

1. First launch shows Ichoi, the terms link, and the privacy link. The user must accept
   the terms before the server screen.
2. The user adds a server with **Add my server**. The app validates the URL and explains
   private HTTP consent.
3. The app lists albums, playlists, and tracks. Playback loads one track into memory for
   `AVAudioPlayer`; it does not save a media file.
4. LinkKeys sign-in opens the server flow. The exchange code is sent to
   `SessionService.authenticate`. The returned token is stored in the Keychain for that
   server only. The server can return only to the allow-listed `ichoi://linkkeys` native
   callback or to its own browser page.
5. Settings shows the selected server URL, privacy and terms links, and **Sign out**.
6. Account deletion requires an exact copy of the current handle. The app removes the
   local token only after a successful server response. If the request fails, it keeps the
   session and shows the selected server URL and administrator guidance.
7. Signed-in users can report a playlist or account. Reasons are objectionable content,
   harassment, spam, and other. Details are optional and limited to 2,000 Unicode scalar
   values.
8. Signed-in users can block a playlist owner. Settings lists blocked accounts and permits
   unblock. A block applies only to the selected server.

The app has no account creation form. LinkKeys login can create an account on the selected
server. The server operator controls account data and deletion.

## Release build

Run the release from Linux. The script copies the iOS tree to the macOS build host. It
pipes the App Store Connect API key from the Reactorcide secret path `nsc/apple`:

```sh
ios/scripts/remote-release.sh 0.1.0 export   # sign an App Store package only
ios/scripts/remote-release.sh 0.1.0 upload   # sign and upload to App Store Connect
```

The default host is `todhansmann@10.19.81.206`. Set `ICHOI_MAC_HOST` to use a different
host. On the host, `ios/scripts/release-build.sh` runs the static checks and `swift test`. It
then archives and exports with automatic signing. The build number is
major × 10000 + minor × 100 + patch. App Store Connect must contain the app record for
`com.catalystsquad.ichoi` before an upload.

## macOS verification checklist

Run these commands from the repository root on a Mac with Xcode installed:

```sh
cd ios
sh scripts/check-compliance.sh
swift test
CONFIGURATION=Debug PRODUCT_BUNDLE_IDENTIFIER=com.catalystsquad.ichoi.dev sh scripts/verify-release.sh
PRODUCT_BUNDLE_IDENTIFIER=com.catalystsquad.ichoi.dev CONFIGURATION=Release sh scripts/verify-release.sh
PRODUCT_BUNDLE_IDENTIFIER=com.catalystsquad.ichoi CONFIGURATION=Release sh scripts/verify-release.sh
xcodebuild -project Ichoi.xcodeproj -scheme Ichoi -configuration Debug -sdk iphonesimulator build
xcodebuild -project Ichoi.xcodeproj -scheme Ichoi -configuration Release -sdk iphoneos archive -archivePath build/Ichoi.xcarchive
```

The Release guard command with the development ID must fail. The Release guard command
with the permanent ID must pass. Inspect the bundle ID in the archive export.

Use Xcode 26 or later and the iOS 26 SDK or later. This is the minimum App Store upload
requirement from April 28, 2026. Use the current supported Xcode release when practical.

Check these items on a supported iPhone and simulator:

- generated Swift files are present in the Xcode build and are unchanged after `./tools.sh gen`;
- signing and entitlements contain only the required app capabilities;
- background audio continues after the screen locks, and the playback controls work;
- HTTPS works with a matching pin, while a mismatch and a downgrade redirect fail;
- private HTTP works only after consent, and public HTTP always fails;
- two server profiles keep different tokens; sign-out removes only the selected token;
- first-run terms acceptance persists, and account deletion keeps the token on failure;
- account deletion removes all playlists that the account owns;
- block and unblock hide and restore public playlists for only the selected server profile;
- report limits count Unicode scalars; no media file appears in shared or app-private storage;
- the archive contains `PrivacyInfo.xcprivacy` and the approved app icon.

Device testing, archive export, App Store Connect metadata, age rating, privacy nutrition
labels, screenshots, support URL, marketing URL, contact email, export compliance, and
final legal approval are external. The release owner must also complete the content-rights
declaration and the Digital Services Act trader-status declaration. Complete the current
age-rating questionnaire, including its user-content and social-capability questions. Do
not select **Made for Kids**. Use an age-rating override only after the legal owner approves
it.

App Review needs full access to account functions. Supply the working demo server, the
dedicated store-review LinkKeys member credential, and clear review steps. Verify report,
block, unblock, and account deletion before submission. A later login recreates the trusted
reviewer account after deletion. Explain that LinkKeys signs a user in to the selected
third-party service. Explain why the client-service exception in Guideline 4.8 applies. If
Apple does not accept that exception, do not submit until the app has an approved equivalent
login option.

## App Store Connect submission record

Before submission, record the following values in the release ticket (not in this
repository): permanent bundle ID, Team ID, version and build number, signing profile,
privacy URL, terms URL, account deletion URL, support contact, age rating answers, and
export-compliance answer. Also record the content-rights answer, privacy-manifest report,
DSA trader status, demo review steps, and artwork approval. Confirm that all URLs are public
HTTPS pages and that the legal owner approved their content.
