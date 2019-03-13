#include <sys/types.h>
#include <dirent.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>

int main(int argc, const char* argv[]) {
    DIR* dirp = opendir("/");
    if (dirp == NULL) {
        printf("failed to open directory at /\n");
        return -1;
    }

	struct dirent *dp;
	while ((dp = readdir(dirp)) != NULL) {
		printf("get: %s\n", dp->d_name);
	}

    closedir(dirp);

    printf("Read directory test successful\n");
    return 0;
}
