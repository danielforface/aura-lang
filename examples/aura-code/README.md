# aura-code

A tiny interactive Aura + Lumina (raylib) demo.

## Run (dev)

From the repo root:

```powershell
$env:Path = $env:Path + ";C:\\Program Files\\CMake\\bin"
cargo run -p aura --features z3,lumina-raylib -- run examples/aura-code/aura-code.aura --mode avm
```

## Run (AuraSDK)

If you have an AuraSDK zip extracted and `AURA_HOME` set:

```powershell
aura run examples/aura-code/aura-code.aura --mode avm
```

(Ensure the SDK includes `aura-lumina` runtime support on your platform.)
