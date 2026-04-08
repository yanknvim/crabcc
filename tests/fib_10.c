int main() {
    int a;
    int b;
    int i;
    int t;
    a = 0;
    b = 1;
    i = 0;
    for (i = 0; i < 10; i = i + 1) {
        t = a + b;
        a = b;
        b = t;
    }
    return a;
}
