/* Пример POSIX программы, которая может быть скомпилирована с posix-compat */

#include <unistd.h>
#include <string.h>
#include <stdlib.h>

int main(int argc, char *argv[]) {
    const char *msg = "Hello from POSIX-compatible program!\n";
    
    /* Использовать POSIX write(2) — будет транслировано в Hammam syscall */
    write(1, msg, strlen(msg));
    
    /* Использовать POSIX exit(2) */
    exit(0);
    
    return 0;
}
