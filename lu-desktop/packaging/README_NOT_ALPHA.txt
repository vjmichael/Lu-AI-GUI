LU R22 INDEPENDENT DESKTOP — DEVELOPMENT EVALUATION (NOT ALPHA)
================================================================
Windows x64 development evaluation of the R22 typed-chat source from
vjmichael/Lu-AI-GUI. This version supersedes R21 for testing; R21 testing
may be skipped. THIS IS NOT AN APPROVED WINDOWS ALPHA.

WHAT CHANGED IN R22
- User and assistant history is transmitted as typed Lu Protobuf chat turns
  rather than flattening messages into a text transcript with User:/Assistant:.
- Lu routes both normal and image prompts through llama.cpp chat completions,
  allowing the model chat template to preserve role boundaries.
- Better HTTP error reporting for rejected multimodal requests.
- Retains existing conversation queue and cancellation semantics, model import,
  MMProj pairing, image admission, MarkDown formatting and attachment previews.

WINDOWS MORNING ACCEPTANCE
1. If preserving chat history matters, back up %APPDATA%\org.lu.desktop.
   Close R20 and any earlier Lu versions before running this installer.
2. Install R22 and launch. Verify no missing Windows entry-point errors.
3. Choose the existing GGUF/MMProj pair. Load explicitly; merely selecting
   a model does not load it. If a model fails to load, save the exact error.
4. In a fresh chat ask: 'What is the cost of living in Arkansas?'. Cancel
   mid-response, then ask 'What about compared with California?'. Look for
   state-aware context and absence of repeating role tags.
5. In a NEW chat ask 'What Christmas song do people often dislike?'. Confirm
   no unrelated Arkansas or Yukon text appears. This is a check for isolation,
   not an objective fact test; the model may answer imperfectly.
6. Attach a known valid PNG/JPEG and describe a simple feature visible ONLY in
   the image, then ask a follow-up. Verify the actual image reaches Lu and the
   model gives a grounded answer. Record any HTTP diagnostics.
7. Send Chat A and Chat B prompts without canceling A; B should queue and
   then run. Cancel a queued Chat C without affecting A/B. Reopen the app and
   confirm saved turns remain in their correct conversations.
8. Unload GGUF and confirm GPU memory is released. Inspect backend logs if
   available to verify actual AMD Vulkan layer offload, not just VRAM use.
9. Test installation, shutdown, and uninstall. Do not delete user AppData
   directories unless intentionally starting with a fresh profile.

KNOWN LIMITATIONS / UNVERIFIED
- Windows CPU/GPU offload and MMProj vision in R22 have not been physically
  verified; Linux small-model chat smoke checks are not a substitute.
- Native tools and full ANSF recovery integration remain open alpha gates.
- Model-loading percentage, PDF/DOCX parsing and JSON repair are deferred.
- The desktop cross-build uses a verified embedded Common Controls v6 manifest;
  full production Windows resource/version validation is not complete.
- Uninstall does not delete model files, chats, or other user-owned AppData.

For evidence and source see:
https://github.com/vjmichael/Lu-AI-GUI
