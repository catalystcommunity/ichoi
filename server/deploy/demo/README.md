# Demo server deployment template

This directory contains development-only deployment configuration. It does not
contain production secrets or approved media.

Before deployment, a person must select a public host, approve each recording in
`LICENSES.md`, approve the support and privacy contacts, and provide the LinkKeys
values. Run `bash ./check-config.sh` before any deployment command. The check fails
while a reserved domain, placeholder, or unapproved licence entry remains.

After the first administrator signs in, remove `ICHOI_ADMIN_TOKEN`. Trust only
the exact administrator identity. Do not add jukebox outputs to the public demo.
