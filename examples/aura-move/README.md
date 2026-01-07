# aura-move

A small "game-ish" demo showing object movement using Aura + Lumina.

It renders a movable rectangle (`Rect`) and updates its position when you click the D-Pad buttons.

## Run

```powershell
cd examples\aura-move
$env:Path = $env:Path + ";C:\\Program Files\\CMake\\bin"
$env:AURA_UI_DEBUG = "1"
cargo run -p aura --features z3,lumina-raylib -- run aura-move.aura --mode avm
```
