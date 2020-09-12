# Building the shaders

You will need `glslangValidator` and `spirv-cross` installed to execute the
Makefile from this directory. You can speed up the build by parallelizing the
build: `make -j`.

## macOS

You can use [Homebrew](https://brew.sh/) to install the dependencies:

```sh
brew install glslang spirv-cross
```

## Windows

`glslangValidator` and `spirv-cross` are available by installing the
[Vulkan SDK](https://vulkan.lunarg.com/sdk/home). You'll also need some commands
like `make`, `rm`, etc. These are available on the
[Windows Subsystem for Linux](https://docs.microsoft.com/en-us/windows/wsl/install-win10)
shell. You'll need to set these environment variables for `make` to succeed:

```sh
export GLSLANG=glslangValidator.exe
export SPIRVCROSS=spirv-cross.exe
```
