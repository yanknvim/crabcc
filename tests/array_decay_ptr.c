int main() {
    int a[2];
    int *p;
    p = a;
    p[0] = 3;
    p[1] = 9;
    return a[0] + a[1];
}
