Lu native codec resource directory (development source only).

Do not distribute this folder without real vetted Windows libarchive DLLs.
The pinned build stages native archive.dll/libarchive.dll and codec dependencies
using scripts/stage-lu-native-codecs.ps1. Never copy opaque DLLs from PATH.
The runtime fails closed if this app-owned native codec directory has no library.
The release/NSIS acceptance gate must check manifest SHA-256 and licenses.
