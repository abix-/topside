# TOPSIDE: A GREAT PLACE TO LIVE

Fifty homes. Two hundred neighbors. One community.

## Life in Topside

Friendly neighbors. Fresh milk from the farms.
A full grocery. Church on Sunday.
A sheriff who is always on duty.

## Weather

Topside enjoys mild weather most of the year.
Storms are nothing to worry about.

## Do your part

Be a good neighbor. Keep busy.
A lively town is a happy town.

Topside takes care of its own.

*[Would you like to know more?](#topside-a-great-place-to-live)*

---

## Building

topside runs on Windows (DirectX 12) and is built on
[modforge](https://github.com/abix-/modforge). Clone both side by side:

```
git clone https://github.com/abix-/modforge
git clone https://github.com/abix-/topside
cd topside
cargo run --release
```

With the Windows SDK installed, shaders compile with its DXC; without it the
first launch takes about a minute longer. `scripts/build.ps1` is the
development script; it runs cargo through
[k3sc](https://github.com/abix-/k3sc) `cargo-lock`.

## License

GPL-3.0. See [LICENSE](LICENSE).
