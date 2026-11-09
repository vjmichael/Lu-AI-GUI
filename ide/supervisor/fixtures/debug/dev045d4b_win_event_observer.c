/*
 * DEV-04.5D4B disposable Windows CI observer only.
 *
 * Owns a DEBUG_ONLY_THIS_PROCESS fixture. An actual WaitForDebugEvent
 * EXCEPTION_BREAKPOINT is the independent OS stop witness; GetThreadContext
 * without that event is never accepted. No attach, arbitrary target, module
 * mutation or source-level Lu debugger authority is implemented here.
 */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <wchar.h>
#include <stdint.h>

static int stop_child(PROCESS_INFORMATION *p, int code) {
    if (p->hProcess) {
        TerminateProcess(p->hProcess, (UINT)code);
        WaitForSingleObject(p->hProcess, 3000);
    }
    if (p->hThread) CloseHandle(p->hThread);
    if (p->hProcess) CloseHandle(p->hProcess);
    return code;
}
int wmain(int argc, wchar_t **argv) {
#if !defined(_M_X64) && !defined(__x86_64__)
    return 91;
#else
    if (argc != 2 || !argv[1] || wcslen(argv[1]) > 1024) return 90;
    wchar_t full[MAX_PATH] = {0};
    DWORD required = GetFullPathNameW(argv[1], MAX_PATH, full, NULL);
    if (!required || required >= MAX_PATH) return 92;
    STARTUPINFOW startup = {0};
    PROCESS_INFORMATION child = {0};
    startup.cb = sizeof(startup);
    if (!CreateProcessW(full, NULL, NULL, NULL, FALSE,
                        DEBUG_ONLY_THIS_PROCESS | CREATE_NO_WINDOW,
                        NULL, NULL, &startup, &child)) return 93;
    wchar_t actual[MAX_PATH] = {0};
    DWORD actual_size = MAX_PATH;
    if (!QueryFullProcessImageNameW(child.hProcess, 0, actual, &actual_size)
        || _wcsicmp(actual, full) != 0) {
        return stop_child(&child, 94);
    }
    unsigned long long image_base = 0;
    int create_seen = 0;
    for (int attempt = 0; attempt < 256; ++attempt) {
        DEBUG_EVENT event = {0};
        if (!WaitForDebugEvent(&event, 10000)) {
            return stop_child(&child, 95);
        }
        if (event.dwProcessId != child.dwProcessId) {
            ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE);
            return stop_child(&child, 96);
        }
        if (event.dwDebugEventCode == CREATE_PROCESS_DEBUG_EVENT) {
            if (create_seen || !event.u.CreateProcessInfo.lpBaseOfImage) {
                ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE);
                return stop_child(&child, 97);
            }
            create_seen = 1;
            image_base = (unsigned long long)(uintptr_t)
                event.u.CreateProcessInfo.lpBaseOfImage;
            if (event.u.CreateProcessInfo.hFile)
                CloseHandle(event.u.CreateProcessInfo.hFile);
        } else if (event.dwDebugEventCode == LOAD_DLL_DEBUG_EVENT) {
            if (event.u.LoadDll.hFile) CloseHandle(event.u.LoadDll.hFile);
        } else if (event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT
                   && create_seen
                   && event.u.Exception.ExceptionRecord.ExceptionCode == EXCEPTION_BREAKPOINT) {
            unsigned long long location = (unsigned long long)(uintptr_t)
                event.u.Exception.ExceptionRecord.ExceptionAddress;
            /* Ignore initial Windows loader breakpoint, which is outside
             * the independently observed main PE image. This fixture is
             * fixed and tiny: a 16MiB cap is a defensive upper bound, not
             * a replacement for the Rust SizeOfImage verification.
             */
            if (location >= image_base
                && location - image_base < (16ULL << 20)) {
                HANDLE thread = OpenThread(THREAD_GET_CONTEXT
                    | THREAD_QUERY_LIMITED_INFORMATION, FALSE, event.dwThreadId);
                if (!thread) {
                    ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE);
                    return stop_child(&child, 98);
                }
                CONTEXT context = {0};
                context.ContextFlags = CONTEXT_CONTROL;
                BOOL valid = GetThreadContext(thread, &context);
                CloseHandle(thread);
                if (!valid || context.Rip < image_base
                    || context.Rip - image_base >= (16ULL << 20)) {
                    ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE);
                    return stop_child(&child, 99);
                }
                printf("D4B_OS_DEBUG_EVENT=PASS\n");
                printf("D4B_OS_IMAGE_BASE=0x%llx\n", image_base);
                printf("D4B_OS_EVENT_ADDRESS=0x%llx\n", location);
                printf("D4B_OS_THREAD_RIP=0x%llx\n",
                       (unsigned long long)context.Rip);
                printf("D4B_OS_EXE_IDENTITY=PASS\n");
                fflush(stdout);
                ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE);
                return stop_child(&child, 0);
            }
        } else if (event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT) {
            ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE);
            return stop_child(&child, 100);
        }
        if (!ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE)) {
            return stop_child(&child, 101);
        }
    }
    return stop_child(&child, 102);
#endif
}
