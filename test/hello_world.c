extern long long sys_write(int fd, const char* buf, unsigned long long size);

int main(void) {
    char msg[] = "Hello, World!\n";
    sys_write(1, msg, sizeof(msg));
    return 0;
}
