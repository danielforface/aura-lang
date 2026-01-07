# Aura by Example (Cookbook)

Short, copy/pasteable examples that match the current prototype.

## Hello, output

```aura
import std::io

cell main() ->:
    io.println("hello")
```

## While loop with invariant

```aura
cell main() ->:
    val mut i: u32 = 0
    while i < 10 invariant i <= 10:
        i = i + 1
```

## Match exhaustiveness (wildcard last)

```aura
cell main() ->:
    val x: u32 = 2
    match x:
        0: io.println("zero")
        1: io.println("one")
        _: io.println("many")
```

## Tensor usage (shape-safe patterns)

```aura
import aura::tensor

cell main() ->:
    val t: Tensor<u32, [2, 2, 3]> = tensor::new<u32>(12)
    t ~> tensor::set(0, 255)
    t ~> tensor::set(1, 128)
    t ~> tensor::set(2, 64)
```

## Unsafe boundary (FFI calls)

```aura
import std::core

extern cell do_low_level_thing(x: u32) -> u32

cell main() ->:
    unsafe:
        val y: u32 = do_low_level_thing(1)
        core.debug("done")
```

## Async lambda capture rule (no mutable capture)

This should be rejected by sema:

```aura
cell main() ->:
    val mut x: u32 = 0
    val f = ~> { x = x + 1 }
```
