#include <unistd.h>
#include <stdlib.h>
#include <string.h>
#include <libgen.h>
#include <limits.h>
#include <stdio.h>

int main(int argc, char *argv[]) {
    char path[PATH_MAX];
    char script[PATH_MAX];

    // Get directory of this executable
    if (realpath(argv[0], path) == NULL) {
        return 1;
    }

    char *dir = dirname(path);

    // Build path to the shell script launcher
    snprintf(script, sizeof(script), "%s/lfv-launcher.sh", dir);

    // Exec the shell script, passing through any args
    execl("/bin/bash", "bash", script, NULL);

    // If exec fails, try execing the lfv binary directly as fallback
    snprintf(script, sizeof(script), "%s/lfv", dir);
    execl(script, "lfv", "tui", NULL);

    return 1;
}
