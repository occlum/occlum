volatile int g_int = 0;
static void use_int(int *a) {
    g_int += *a;
}

__thread int tls_g_int = 0;

int main(int argc, const char *argv[]) {
    use_int(&tls_g_int);
    return g_int;
}
