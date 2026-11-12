//===-- LuIdeOwnerEvent.h --------------------------------------------------===//
//
// First-party Lu IDE LLVM 18.1.8 Windows debug-event sideband.
 // May serve a strictly read-only pinned release observer ONLY after
 // independent build-time source/binary provenance and native OS acceptance.
 // No production stepping, breakpoint, variables or replay authority.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// This header MUST be applied only to the exact pinned upstream LLVM tag.
// Not a general debugger attach API.  The host passes a one-use, pre-created
// inherited pipe handle and 128-bit nonce through the native, restricted
// process environment.  No source, address, PID or command is accepted from
// an AI model or DAP JSON.  Fail-closed on any configured-channel failure.
//
#pragma once
#ifndef _WIN32
#error Lu IDE owner-event hook is Windows-only
#endif
#include <windows.h>
#include <cstdint>
#include <cstring>

namespace lu_ide_owner {
#pragma pack(push, 1)
struct EventRecord {
  char magic[8];                 // "LUOE0001"
  uint8_t nonce[16];             // fresh host admission nonce
  uint64_t sequence;             // one sequence per kernel exception event
  uint32_t kind;                 // 1=RECEIVED while owned; 2=CONTINUED
  uint32_t process_id;
  uint32_t thread_id;
  uint32_t debugger_owner_thread_id;
  uint32_t exception_code;
  uint32_t first_chance;
  uint64_t thread_rip;
  uint64_t exception_address;
  uint32_t continue_status;      // 0=received; 1=continued OK; 2=failed
};
#pragma pack(pop)
static_assert(sizeof(EventRecord) == 76, "owner wire layout changed");

[[noreturn]] inline void Deny() {
  // Do not let an unattested debuggee continue under a configured Lu
  // attestation mode. The preexisting Job still owns all descendants.
  ::TerminateProcess(::GetCurrentProcess(), 124);
  ::ExitProcess(124);
}
inline int Hex(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}
class Owner {
  HANDLE pipe_ = INVALID_HANDLE_VALUE;
  uint8_t nonce_[16] = {};
  bool enabled_ = false;
  uint64_t next_sequence_ = 0;
  uint64_t active_sequence_ = 0;
  DWORD active_process_id_ = 0;
  DWORD active_thread_id_ = 0;
  uint64_t active_rip_ = 0;
  uint64_t active_address_ = 0;
  DWORD active_exception_code_ = 0;
  DWORD active_first_chance_ = 0;

  Owner() {
    char handle_text[32] = {};
    char nonce_text[40] = {};
    DWORD hn = ::GetEnvironmentVariableA(
        "LU_IDE_OWNER_PIPE_HANDLE", handle_text, sizeof(handle_text));
    DWORD nn = ::GetEnvironmentVariableA(
        "LU_IDE_OWNER_NONCE", nonce_text, sizeof(nonce_text));
    if (!hn && !nn) return; // ordinary upstream LLDB behavior, no hook
    if (!hn || hn >= sizeof(handle_text) || nn != 32) Deny();
    uintptr_t raw = 0;
    for (DWORD i = 0; i < hn; ++i) {
      int h = Hex(handle_text[i]);
      if (h < 0 || raw > (UINTPTR_MAX >> 4)) Deny();
      raw = (raw << 4) | static_cast<uintptr_t>(h);
    }
    if (!raw || raw == UINTPTR_MAX) Deny();
    for (unsigned i = 0; i < 16; ++i) {
      int hi = Hex(nonce_text[i * 2]), lo = Hex(nonce_text[i * 2 + 1]);
      if (hi < 0 || lo < 0) Deny();
      nonce_[i] = static_cast<uint8_t>((hi << 4) | lo);
    }
    pipe_ = reinterpret_cast<HANDLE>(raw);
    // Prevent this private write capability from being inherited by the
    // inferior when LLDB's ProcessLauncherWindows creates the debuggee.
    if (!::SetHandleInformation(pipe_, HANDLE_FLAG_INHERIT, 0)) Deny();
    enabled_ = true;
  }
  void Send(DWORD kind, DWORD pid, DWORD tid, DWORD code, DWORD first,
            uint64_t rip, uint64_t address, DWORD continued) {
    if (!enabled_) return;
    EventRecord r = {};
    std::memcpy(r.magic, "LUOE0001", 8);
    std::memcpy(r.nonce, nonce_, 16);
    r.sequence = active_sequence_;
    r.kind = kind;
    r.process_id = pid;
    r.thread_id = tid;
    r.debugger_owner_thread_id = ::GetCurrentThreadId();
    r.exception_code = code;
    r.first_chance = first;
    r.thread_rip = rip;
    r.exception_address = address;
    r.continue_status = continued;
    DWORD wrote = 0;
    if (!::WriteFile(pipe_, &r, sizeof(r), &wrote, nullptr) ||
        wrote != sizeof(r)) Deny();
  }

public:
  Owner(const Owner &) = delete;
  Owner &operator=(const Owner &) = delete;
  static Owner &Instance() {
    static Owner owner;
    return owner;
  }
  void OnReceived(const DEBUG_EVENT &event) {
    if (!enabled_ || event.dwDebugEventCode != EXCEPTION_DEBUG_EVENT)
      return;
    // A new exception may never reuse an active, unrevoked owner receipt.
    if (active_sequence_ != 0 || next_sequence_ >= 4096) Deny();
    HANDLE h = ::OpenThread(THREAD_GET_CONTEXT | THREAD_QUERY_LIMITED_INFORMATION,
                            FALSE, event.dwThreadId);
    if (!h) Deny();
    CONTEXT context = {};
    context.ContextFlags = CONTEXT_CONTROL;
    BOOL ok = ::GetThreadContext(h, &context);
    ::CloseHandle(h);
    if (!ok || context.Rip == 0) Deny();
    active_sequence_ = ++next_sequence_;
    active_process_id_ = event.dwProcessId;
    active_thread_id_ = event.dwThreadId;
    active_rip_ = static_cast<uint64_t>(context.Rip);
    active_address_ = reinterpret_cast<uint64_t>(
        event.u.Exception.ExceptionRecord.ExceptionAddress);
    active_exception_code_ = event.u.Exception.ExceptionRecord.ExceptionCode;
    active_first_chance_ = event.u.Exception.dwFirstChance;
    Send(1, active_process_id_, active_thread_id_, active_exception_code_,
         active_first_chance_, active_rip_, active_address_, 0);
  }
  void OnContinued(const DEBUG_EVENT &event, BOOL succeeded) {
    if (!enabled_ || event.dwDebugEventCode != EXCEPTION_DEBUG_EVENT) return;
    if (!active_sequence_ || active_process_id_ != event.dwProcessId ||
        active_thread_id_ != event.dwThreadId) Deny();
    Send(2, active_process_id_, active_thread_id_, active_exception_code_,
         active_first_chance_, active_rip_, active_address_,
         succeeded ? 1 : 2);
    active_sequence_ = 0;
  }
};
inline void OnReceived(const DEBUG_EVENT &event) {
  Owner::Instance().OnReceived(event);
}
inline void OnContinued(const DEBUG_EVENT &event, BOOL succeeded) {
  Owner::Instance().OnContinued(event, succeeded);
}
} // namespace lu_ide_owner
