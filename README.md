# Mi Band 4 - GTK UI

![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/grimsteel/miband4-gtk/release.yml)


A GTK interface for interacting with your Mi Smart Band 4.

See also: https://github.com/grimsteel/miband4-web

## Installation

[![AUR Badge](https://img.shields.io/aur/version/miband4-gtk-bin)](https://aur.archlinux.org/packages/miband4-gtk-bin)

There are prebuilt binaries for `x86_64-unknown-linux-gnu` on the Releases page.

## Building from Source

You'll need `gtk4` (version 4.10 or higher) (`libgtk-4-dev`) installed.

```sh
cargo build
```


## Features

* Current Activity Data
* Time
* Battery
* Music (syncs with MPRIS using `playerctld`)
* Notifications (uses `org.freedesktop.Notifications`)
* Band Lock

![image](https://github.com/user-attachments/assets/5240d071-1f5c-4e9c-b829-71d68b7d5921)
