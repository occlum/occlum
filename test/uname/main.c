#include <sys/utsname.h>
#include <stdio.h>

int main(void) {
    struct utsname name;
    uname(&name);
    printf("sysname = %s\n", (const char *)&name.sysname);
    printf("nodename = %s\n", (const char *)&name.nodename);
    printf("release = %s\n", (const char *)&name.release);
    printf("version = %s\n", (const char *)&name.version);
    printf("machine = %s\n", (const char *)&name.machine);
    printf("domainname = %s\n", (const char *)&name.__domainname);
    return 0;
}
