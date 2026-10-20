# Demo server deployment template

This directory contains development-only deployment configuration. It does not
contain production secrets or approved media.

Before deployment, a person must select a public host, approve each recording in
`LICENSES.md`, approve the support and privacy contacts, and provide the LinkKeys
values. Run `bash ./check-config.sh` before any deployment command. The check fails
while a reserved domain, placeholder, or unapproved licence entry remains.

The first admitted LinkKeys identity becomes the administrator. Trust only the
exact administrator login. Set `ICHOI_ANONYMOUS_JUKEBOX=true` to let guests
create and control ownerless browser jukebox shares. This setting does not let
guests save playlists, save progress, copy media, use administration tools, or
control server and satellite speakers.
Set `ICHOI_ZERO_ACCOUNT_GUEST_ACCESS=false` so these restrictions apply before
the first administrator login.
