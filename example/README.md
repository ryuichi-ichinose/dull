# dull example

Four images: `base` writes one file per app under `/opt/base` at build time;
`app2` and `app3` are built on `base` and each print their file; and
`app1` does not depend on `base`.

## Steps

```console
$ dull init                      # create dull.toml
$ dull venv                      # set up this project's isolated storage
$ dull scan                      # register base, app1, app2, app3 as [images.X]
```

Then edit `dull.toml`:

1. Set `dependencies` (the first line, written by `scan` as `dependencies = []`):

   ```toml
   dependencies = ["quay.io/fedora/python-312:latest", "quay.io/libpod/registry:2.8.2"]
   ```

2. Add a build group so `base` is built before `app2` and `app3`, at the end of the file:

   ```toml
   [build_groups.apps]
   base = "base"
   images = ["app2", "app3"]
   ```

Then continue:

```console
$ dull lock                      # resolve python-312 and the registry image to digests in dull.lock
$ dull fetch                     # pull that digest into project storage
$ dull build -a                  # build all four images
```

Check the result:

```console
$ dull images                    # Lock, State, and local images in this project
```

Run an image from inside the project's storage:

```console
$ dull shell                     # enter the shell for this project's storage
$ podman run --rm localhost/app1:latest
$ podman run --rm localhost/app2:latest
$ podman run --rm localhost/app3:latest
$ exit
```

## Push to a registry

Run a registry container from the locked image, then push all images to it:

```console
$ dull shell                     # enter the shell for this project's storage
$ podman run -d --name example-registry -p 5000:5000 quay.io/libpod/registry:2.8.2
$ dull registry add local localhost:5000
$ dull push --all -r local --insecure   # --insecure: this registry serves plain HTTP. Do not do this in real use!
$ curl http://localhost:5000/v2/_catalog # lists the pushed repositories
$ podman rm -f example-registry
$ exit
```

## Clean up

Remove the images this example built, then the project's storage:

```console
$ dull rmi --all                 # remove every image in this project's storage
$ buildah unshare rm -rf .dull/  # remove the storage itself (rootless files need unshare)
```

