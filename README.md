# winter-lily

A Linux->Lilium Compatibility layer (like Wine). 

## Elf Loader

The primary component of winter-lily is a custom ELF Loader, wl-ld-lilium.so. This behaves like ld-lilium.so but can also load native (Linux) shared libraries. 

On top of the standard Lilium rtld interface, it exposes the following additional symbols:
* `__wl_rtld_open_native` (used by `kmgmt:OpenKModule`).

Note that both lillum *and* native libraries cannot have any `PF_W | PF_X` segments (`PT_LOAD` or `PT_GNU_STACK`). Also, regardless of `PT_GNU_STACK`, the stack will never be executable when mapped.

Due to limitations in the implementation, Lilium currently does not handle relocating read-only segments. This includes textrel and relro. 
This will be fixed in the future.
However, it does support both eager and lazy plt binding.

## 