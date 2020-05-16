# Pathfinder C bindings

To generate these bindings, you first need to install [cargo-c](https://crates.io/crates/cargo-c).

* On ArchLinux:
  ```
  # pacman -S cargo-c
  ```
* Anywhere else:
  ```
  $ cargo install cargo-c
  ```

Now you can generate the C header, library and pkg-config file, and install them for your user using this command:
```
$ cargo cinstall --prefix=$HOME/.local
```

Or if you want to install it system-wide:
```
# cargo cinstall --prefix=/usr/local
```

Another option, for packagers, is to install it to a temporary directory then put its content in a package:
```
$ cargo cinstall --prefix=/usr --destdir=$XDG_RUNTIME_DIR/staging
```
