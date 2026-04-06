# winctl - source

Window management CLI using Win32 APIs.

## Build

Requires .NET 10+ SDK.

```bash
cd src/winctl
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:PublishTrimmed=true -p:IncludeNativeLibrariesForSelfExtract=true -o ../../skills/winctl/bin
```

## Requirements

- Windows 10/11
