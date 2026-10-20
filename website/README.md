# Ichoi project website

This directory contains the Ichoi static website. Edit files in `site-src/`.
Pysocha writes the deployable output to `site/`. Do not edit `site/` by hand.

## Build

The build uses Pysocha at commit `18b2d6e704ba63e80f82d287b5831c1151c388a4`:

```sh
./tools.sh build
```

Run the command twice. The second run must make no changes. Preview the result
with `python3 -m http.server 4173 --directory site`.

## Deployment

The site is served at `https://catalystichoi.com`. Reactorcide deploys it when
`VERSION.txt` changes on `main`. Raise the version in the same pull request as
the change, or in a later one. A change without a version change merges but
does not deploy.

`./tools.sh check-deploy` fails while a reserved `.invalid` domain or a
`REPLACE-ME` value exists. The image build runs the same check.

The pysocha-site chart makes an Ingress. The deploy job turns it off and
applies `httproute.yaml`, because the Ingress conflicts with the cluster
Gateway listeners. See `.reactorcide/BOOTSTRAP.md`.
