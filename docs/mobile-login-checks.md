# Mobile sign-in checks

The **Blocked accounts** list shows accounts that the current user has blocked.
It does not show whether the current user is blocked.

## Android

`SettingsNavigationTest` uses Robolectric and a local WebSocket server. It checks
the LinkKeys callback, late account-list responses, and screen navigation.

Use Java 17 and the Android SDK to run the checks from `android/`:

```sh
JAVA_HOME=/usr/lib/jvm/java-17-openjdk \
ANDROID_HOME="$HOME/Android/Sdk" \
./gradlew test lint assembleDebug androidCompliance
```

Run the device checks in `android/PHYSICAL_DEVICE_TESTS.md` before release.

## iOS

On a Mac with Xcode, run `swift test` and `sh scripts/check-compliance.sh` from
`ios/`. Build the Ichoi scheme with code signing disabled for a compile check.

On an iPhone or an iOS simulator:

1. Open Settings and sign in with LinkKeys. Confirm that Settings closes and the
   library shows the playlists available to that account.
2. Cancel sign-in. Confirm that Settings stays open and the app stays signed out.
3. Open Settings after sign-in. Confirm that the account name appears. Confirm
   that the blocked-account list shows a loading indicator before the response.
4. Make the blocked-account request fail. Confirm that an error appears and the
   app does not say that the list is empty.
5. Remove a block and close Settings. Confirm that the library list updates.
6. Sign out, then select a second server. Confirm that the library uses the
   selected server and account.

Record the device, OS version, app version, date, and result of each check.
