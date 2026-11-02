Lu R14 — Windows physical-machine acceptance test (NOT AN ALPHA APPROVAL)

1. Install Lu_R14_Independent_Desktop_DEV_EVAL_NOT_ALPHA.exe on your Windows 11 PC.
2. Extract this acceptance kit ZIP and double-click START_WINDOWS_LU_R14_TEST.cmd.
3. The script checks the desktop GUI subsystem, signed-off SHA-256 component
   digests, Vulkan device enumeration, and initial process liveness.
4. In the opened Lu window, import a GGUF and test normal text generation.
   Unload, attach its matching mmproj, reload, and test a question whose answer
   depends on an image. A deterministic fixture is included:
   Lu_R14_Vision_Test_Fixture.png. Ask: "What color is the triangle and what
   shape is in the bottom-right?" Expected: yellow triangle; purple square.
   An answer must refer to the actual image, not merely repeat the prompt. Try cancellation and chat restart/recovery.
5. Answer the script's observation prompts, close Lu, and press Enter so it can
   inspect whether llama-server remains running.
6. The script prints an evidence directory, by default under
   %%LOCALAPPDATA%%\LuAcceptance\R14-<timestamp>. Share only the generated
   acceptance-observations.json and llama-list-devices.txt if desired.

Important limits:
- This kit is for real Windows execution, NOT a simulated Windows test.
- GPU enumeration does not prove model layer offload. For GPU acceptance we
  still require logs produced when the actual GGUF is loaded on your AMD GPU.
- The build remains a development evaluation; R14 embeds a real Windows manifest,
  but full Windows resource/icon processing and Lu/ANSF/native-tool acceptance are not complete.
- The script never copies private chat contents, images, model weights or API keys.
- The installer does not bundle Microsoft WebView2 Runtime. If absent, the
  Tauri window may fail to appear; check Windows Event Viewer and WebView2.
