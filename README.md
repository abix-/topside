# TOPSIDE: A GREAT PLACE TO LIVE

Fifty homes. Two hundred neighbors. One community.

## Life in Topside

A bike trail that loops the whole town.\
A corner cafe where they know your order.\
A diner open late, and a bar with live music on Fridays.\
Card nights, porch nights, and a fire pit out past the farms.\
A garden bed for anyone who wants one.\
Somewhere to go and someone to see, every day of the week.

## Getting outside

Quiet streets made for walking and riding.\
Open fields at the edge of town.\
Long evenings, and more stars than you have ever seen.

## Weather

Warm days and cool nights.\
Storms pass quickly here.

## Your neighbors

Everyone here knows your name.\
Everyone here remembers you.

## Do your part

Say hello. Join in. Keep busy.\
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
