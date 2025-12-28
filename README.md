## About

Dyalog APL 20.0's APLAN/⎕VGET allows for a unique way of handling foreign library calling. 

This repo is dedicated to (i) learning libffi through a rust implementation of a wrapper over it in `src/lib.rs` and (ii) imagining what a foreign library caller would look like utilizing APLAN.

This is highly experimental and nothing is guaranteed, I have not guaranteed edge cases working since this for testing. Because of this, I don't recommend using this for your projects.

## Reasoning

Consider this struct definition in SDL3 GPU:
```c
typedef struct SDL_GPUColorTargetInfo
{
    SDL_GPUTexture *texture;
    Uint32 mip_level;
    Uint32 layer_or_depth_plane;
    SDL_FColor clear_color;
    SDL_GPULoadOp load_op;
    SDL_GPUStoreOp store_op;
    SDL_GPUTexture *resolve_texture;
    Uint32 resolve_mip_level;
    Uint32 resolve_layer;
    bool cycle;
    bool cycle_resolve_texture;
    Uint8 padding1;
    Uint8 padding2;
} SDL_GPUColorTargetInfo;
``` 

When this library is used normally, the struct can be defined like this with default initialization: 
```c
SDL_GPUColorTargetInfo colorTargetInfo{};
colorTargetInfo.clear_color = {255/255.0f, 219/255.0f, 187/255.0f, 255/255.0f};
colorTargetInfo.load_op = SDL_GPU_LOADOP_CLEAR; 
colorTargetInfo.store_op = SDL_GPU_STOREOP_STORE;
colorTargetInfo.texture = texture; 
```

While with ⎕NA it would currently have to account for all struct elements when calling a function that takes this struct (ignoring any possible alignment issues).

In my project, I have to handle it like this: 
```
color_info ← ⊂(swap_texture 0 0 (0.1 0.2 0.3 1) 1 0 0 0 0 0 0 0 0)

pass ← lagl.SDL_BeginGPURenderPass cmd_buf (color_info) 1 (depth_info)
```

If we have an idea of what the types are supposed to be and what functions take, a wrapper can be made where it could look something like this instead with APLAN:

```
color_info ← (
    texture: swap_texture
    clear_color: (r: 0.1 ⋄ g: 0.2 ⋄ b: 0.3 ⋄ a: 1)
    load_op: 1
)

pass ← lagl.SDL_BeginGPURenderPass cmd_buf (color_info) 1 (depth_info)
```
Where the function instead is a auto-generated wrapper one over the raw one that unpacks the namespace in order, utilizing ⎕VGET to set defaults and reading out namespaces to call the function with.

This repo is testing to see if that's possible 

## Example

`./TestLib/src/lib.c`
```
typedef struct Inner {
    uint32_t a;
    uint32_t b;
} Inner;

uint32_t fn_struct(Inner s) { return s.a + s.b; }
``` 
Can be currently done as:
```
⎕SE.Link.Create 'ffiw' './ffiw'
⎕IO ← 0

inner ← ffiw.Build_Struct ['a' ffiw.types.u32 ⋄ 'b' ffiw.types.u32]
⎕FX ffiw.Build_Function 'fn_struct' ['s' inner⋄] ffiw.types.u32
⎕ ← 'fn_struct', fn_struct(s: (a: 1 ⋄ b: 2))
```
