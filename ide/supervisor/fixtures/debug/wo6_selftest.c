/* WO6 fixed first-party native debugger acceptance fixture.
 * Compiled with debug symbols and sealed into the reviewed LLDB-DAP bundle.
 * It accepts no user input, file path, network configuration or command.
 * Its only purpose is to provide an observable stop-on-entry, one thread,
 * and a short call stack inside the protected Job/cgroup.
 */
static volatile int fixed_number = 6;
static int add_one(int value) {
    volatile int result = value + 1;
    return result;
}
int main(void) {
    volatile int observed = add_one(fixed_number);
    return observed == 7 ? 0 : 42;
}
