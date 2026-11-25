# topside

**TOPSIDE: A GREAT PLACE TO LIVE.**

Fifty homes. Two hundred neighbors. One community.

## Storm ready

Storms are a normal part of life in Topside.
When the siren sounds, go inside. If your home has a bunker, go down.

Five households have been selected for bunkers.
Bunker space is limited.
If your household was not selected, please remain calm
and stay indoors.

## After the storm

It is normal to feel confused after a storm.
Streets may not be where you left them.
The sky may look different.
Your neighbors may not be the neighbors you remember.

This is normal. Report anything unusual to the Sheriff.

## Do your part

Ration your water. Help your neighbors. Keep busy.
A lively town is a happy town.

Shelter guarantees survival.

*[Would you like to know more?](#topside)*

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
