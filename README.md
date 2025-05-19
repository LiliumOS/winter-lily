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

## Winter Lily Subsystem

The `wl-native-subsys` (`a22304af-3619-59d8-9a95-1335d8e45441`) extension subsystem is loaded by default in every program ran by winter-lily. It does not have a fixed subsystem number and must be queried by using its subsystem ID for a `SysInfoRequestAvailableSubsystem` to determine the subsystem number, version, and supported syscalls.

The `wl-native-subsys` extension allows managing winter-lily, and executing Linux native code. It is only available on winter-lily, and no attempts to emulate it on Lilium itself will be made. 

See the knum description for the system calls exposed by wl-native-subsys. The system calls are also available by name in a dynamically linked program.

## Limitations

The Implementation of Lilium is not complete. It is designed to be mostly compatible with the kernel and default USI and execute most programs. However, a few limitations are placed on programs supported

### Permissions

winter-lily does not attempt to emulate Lilium's permission system, instead relying on Linux's far less granular (and far more limited) permission system. 

As a consequence, the security operations (in the base subsystem) are not reliable. Operations that modify a security context are no-ops currently, and operations for testing permissions do not necessarily return any particular value. 

This may change in the future, or may be moved into a separate project, using controls like 