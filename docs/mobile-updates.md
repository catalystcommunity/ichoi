# Mobile test updates

Checked on 2026-11-11.

## Android

Use a Google Play internal or closed testing track. Each tester joins the test
once and installs Ichoi with the enrolled Google account. Keep automatic updates
on for Ichoi in Google Play. Later releases use the same store listing. Testers
do not need a new invitation for each release.

The app checks Google Play when it enters the foreground. An available update
shows a Download button. Google Play asks for consent and downloads the update
in the background. When the download is complete, the app shows Restart to
install. The app asks before it restarts because a restart interrupts local
playback. The library stays available if the update check fails.

Menu > App and updates shows the installed version, application ID, installer,
and signing certificate fingerprint. It also has a manual update check and a
link to Google Play. Direct installations use the store link. The app does not
fetch or install APK files itself.

### Release procedure

1. Build a signed AAB with `android/scripts/release-bundle.sh X.Y.Z`. Use a version
   code higher than all codes already available to the test group. The script
   derives the code from the version. It cannot read the current Play track.
2. Upload the AAB to the existing `com.catalystsquad.ichoi` app in Play Console.
   Use the existing app signing setup. The upload certificate and the app
   signing certificate have different purposes.
3. Publish the release to the track in which the testers are enrolled. Confirm
   that the release is available, not a draft or a pending publication.
4. Test an update on a device with the previous Play release. Keep the installed
   app and its data. Check server selection, sign-in, queue state, and playback.
5. Check Download, Later, and Restart to install with a newer Play release.
   Unit tests use Google's fake update manager. They do not prove track access
   or the signed installation on a physical device.

### If an update fails

Record the exact Play error and the values from App and updates before removing
anything. Check the following items in Play Console and on the device:

- The Google account is enrolled in the track that contains the release.
- An internal tester is not expecting a closed-track release. Google excludes
  internal testers from open and closed tracks until they leave the internal test.
- The new version code is higher than the installed code.
- The application ID and app signing certificate match the installed app.
- The previous installation did not come from a different signing setup, such
  as a direct APK or Internal App Sharing.

The tester reported only "update failed". This message does not identify the
cause. The reported installation failure has no confirmed cause yet. A client update
check cannot repair a signing mismatch or change test-track membership. Do not
use uninstall and reinstall as the normal update procedure.

Sources: [Play testing tracks](https://support.google.com/googleplay/android-developer/answer/9845334),
[Android update requirements](https://developer.android.com/google/play/app-updates),
[Play update integration](https://developer.android.com/guide/playcore/in-app-updates/kotlin-java),
[Play update tests](https://developer.android.com/guide/playcore/in-app-updates/test).

## iOS

Use TestFlight for beta distribution. Assign new builds to the tester group.
Testers select Ichoi in TestFlight and turn on Automatic Updates. They do not
need to accept another invitation for each build. A TestFlight build expires
90 days after upload. Publish the next build before this limit.

Menu > App and updates shows the installed version and links to TestFlight help.
TestFlight manages beta updates. The App Store manages production updates.

Source: [Apple TestFlight help](https://testflight.apple.com/).

## Next Actions

Publish a release to the existing test track. Test an update from the previous
Play release on a physical phone. Compare the installed version and signing
certificate with the Play release if "update failed" occurs again.
