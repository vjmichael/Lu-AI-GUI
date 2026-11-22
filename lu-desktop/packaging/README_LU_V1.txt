LU CORE v1.0 — WINDOWS NSIS RELEASE-CANDIDATE PACKAGE
====================================================

Product: Lu
Version: 1.0.0
Identity: org.lu.desktop
Source: exact commit and SHA-256 recorded in BUILD_PROVENANCE.json
Windows installer: per-user NSIS, no administrator access required

This installer contains Lu Core (standalone Tauri/Rust desktop) plus
source-pinned native CPU and Vulkan llama-server executables, reviewed
Chromium/Node/Playwright browser runtime, and native host tools.

A local GGUF model and matching MMProj are not included. You supply
your own legally obtained model files. No permission to run a tool or
open the network is granted merely by installation.

This Windows EXE is not Authenticode-signed. If the accompanying
GitHub Release contains an ECC-GPG detached signature (.asc), verify
that signature against the owner's separately trusted public key
and compare the published SHA256SUMS. If no .asc is attached, the
package has no verifiable owner GPG signature. SmartScreen may warn
because Windows code signing is not present.

A passing automated Windows installer test is not evidence that the
owner's exact desktop hardware, personal GGUF models, or all native
tool workflows have been tested. The prior physically accepted Lu
0.1.0-alpha.2 confirmed Radeon AI PRO R9700 GGUF+MMProj capability;
the current package still requires its final ordinary-use walkthrough.

Settings and runtime data are held separately from application files.
The uninstaller must not delete existing user chats, models, or
persistent journals under AppData. Keep a backup before upgrading.

Optional NVP, IDE, Cognitum and Writing DLCs are not part of this
standalone v1.0 distribution.
