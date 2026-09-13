# Google Play submission checklist

Do not submit until every external item has an owner and evidence.

## Repository gates

- [ ] Android unit tests pass.
- [ ] Android lint passes.
- [ ] The debug build passes.
- [ ] The release build passes with the permanent application ID.
- [ ] The release guard rejects the development application ID.
- [ ] The manifest permission check passes.
- [ ] The dependency and prohibited-operation checks pass.
- [ ] Playback creates no persistent media file.
- [ ] Tests pass on Android 10 through the current Android release.
- [ ] Server generation is reproducible and the complete server check passes.
- [ ] Browser tests, type checks, and the production build pass.
- [ ] The website build is reproducible.

## External decisions

- [ ] Select and approve the permanent Android application ID.
- [ ] Create the Play Console application with that ID.
- [ ] Select the website and demo domains.
- [ ] Approve privacy and support contacts.
- [ ] Approve the privacy, terms, data, and licence claims.
- [ ] Approve every demo recording and its attribution.
- [ ] Deploy the website and demo without repository placeholder values.
- [ ] Remove the demo bootstrap token after the first administrator signs in.
- [ ] Run and record the public demo smoke test.

## Play Console

- [ ] Recheck the current policy and target API requirements.
- [ ] Set the price to free and add no in-app products.
- [ ] Set the target audience to ages 18 and over.
- [ ] Do not join the Families program.
- [ ] Enter the approved privacy and deletion URLs.
- [ ] Enter the approved listing and reviewer access text.
- [ ] Complete and approve the Data safety form.
- [ ] Complete the content-rating questionnaire and record its result.
- [ ] Declare user-created content and the report process.
- [ ] Upload screenshots taken against the approved production demo.
- [ ] Upload the signed application bundle.
- [ ] Complete a pre-launch report and resolve all blocking findings.
- [ ] Submit for review only after all prior checks pass.
