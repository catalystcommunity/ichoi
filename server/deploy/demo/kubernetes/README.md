# Kubernetes demo deployment

This deployment serves `https://ichoi.catalystsquad.com` through Cloudflare and the
shared Contour Gateway. The route also accepts the previous name,
`ichoi-demo.catalystsquad.com`, and HTTP from the Cloudflare origin connection. It stores application state and demo media on separate
Longhorn volumes.

The deployment needs two secrets in the `ichoi-demo` namespace:

- `regcred` lets Kubernetes pull the private registry image.
- `ichoi-demo-linkkeys` contains the `api-key` key for the Catalyst Squad
  LinkKeys RP service.

Apply the namespace and storage manifests first. Apply `media-loader.yaml`, and
copy the approved `music/` and `audiobooks/` directories to `/media` in the
loader pod. Delete the loader pod after the copy. Apply `app.yaml` last.

The server admits guests for browsing, playback, and ownerless browser jukebox
shares. A guest cannot save playlists, save progress, copy media, control native
outputs, or use administration tools. Only three LinkKeys identities can create accounts.
`tod@todandlorna.com` and `lorna@todandlorna.com` are administrators.
`nsc-demos@catalystlinkkeys.com` is the store-review member account. Its password is in the
Reactorcide secret path `nsc/ichoi-review`. Give it only to store reviewers.
