# screenshot - source

Screenshot capture CLI using GDI+ and PrintWindow APIs.

## Build

Requires .NET 10+ SDK.

```bash
cd src/screenshot
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:PublishTrimmed=true -p:IncludeNativeLibrariesForSelfExtract=true -o ../../skills/screenshot/bin
```

## Requirements

- Windows 10/11
