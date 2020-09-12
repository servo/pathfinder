# Building the shaders

This document describes how to regenerate the shaders used by Pathfinder. Unless
you have modified files in this directory, regenerating the shaders is not
necessary to use Pathfinder or do most kinds of development on it.

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

Note: the Windows versions of `glslangValidator` and `spirv-cross` may change
the line endings of the generated output. Please take care to ensure that
unintended line ending changes aren't accidentally commited, for instance by
[configuring Git to automatically handle line endings](https://docs.github.com/en/github/using-git/configuring-git-to-handle-line-endings#global-settings-for-line-endings).
